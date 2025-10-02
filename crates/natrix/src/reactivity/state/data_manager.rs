//! Manger of the update cycle and signals.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use smallvec::SmallVec;

use super::{HookKey, InnerCtx};
use crate::error_handling::log_or_panic;
use crate::reactivity::render_callbacks::UpdateResult;
use crate::reactivity::state::hook_manager::{HookStore, InsertionOrder, IterSignalList};
use crate::reactivity::statics;

/// The type that will be used to hold the dirty lists.
pub(crate) type HookDepListHolder = SmallVec<[IterSignalList; 2]>;

/// Store some data but use `O` for its `Ord` implementation
#[derive(Debug)]
struct OrderAssociatedData<T, O> {
    /// The data in question
    data: T,
    /// The value to order based on
    ordering: O,
}

impl<T, O: PartialEq> PartialEq for OrderAssociatedData<T, O> {
    fn eq(&self, other: &Self) -> bool {
        self.ordering == other.ordering
    }
}
impl<T, O: Eq> Eq for OrderAssociatedData<T, O> {}

impl<T, O: PartialOrd> PartialOrd for OrderAssociatedData<T, O> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.ordering.partial_cmp(&other.ordering)
    }
}
impl<T, O: Ord> Ord for OrderAssociatedData<T, O> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordering.cmp(&other.ordering)
    }
}

/// The queue processor of the queue
struct HookQueue {
    /// All changed vectors
    vectors: HookDepListHolder,
    /// The queue of the next item in each vector
    queue: BinaryHeap<OrderAssociatedData<(HookKey, usize), Reverse<InsertionOrder>>>,
    /// A temporary next item
    next_item: Option<HookKey>,
    /// Last processed hook to avoid duplicates
    last_hook: Option<HookKey>,
}

/// Iterate over `iter` until it finds a key in `insertion_orders`
fn get_next_valid<T: State>(
    hook_store: &HookStore<T>,
    iter: &mut IterSignalList,
) -> Option<(HookKey, InsertionOrder)> {
    for hook in iter.by_ref() {
        if let Some(order) = hook_store.get_insertion_order(hook) {
            return Some((hook, order));
        }
    }
    None
}

impl HookQueue {
    /// Create a new queue
    fn new<T: State>(hook_store: &HookStore<T>, mut vectors: HookDepListHolder) -> Self {
        let mut queue = BinaryHeap::with_capacity(vectors.len());

        let first_items = vectors
            .iter_mut()
            .enumerate()
            .filter_map(|(index, vector)| {
                let (hook, ordering) = get_next_valid(hook_store, vector)?;
                Some(OrderAssociatedData {
                    data: (hook, index),
                    ordering: Reverse(ordering),
                })
            });
        queue.extend(first_items);

        Self {
            vectors,
            queue,
            next_item: None,
            last_hook: None,
        }
    }

    /// Push a item to be popped next
    fn push_next(&mut self, key: HookKey) {
        if self.next_item.is_some() {
            log_or_panic!("`push_next` called while `next_item` already has item");
        }

        self.next_item = Some(key);
    }

    /// Pop the next item
    fn pop<T: State>(&mut self, hook_store: &HookStore<T>) -> Option<HookKey> {
        if let Some(next) = self.next_item.take() {
            self.last_hook = Some(next);
            return Some(next);
        }

        loop {
            log::trace!("current queue: {:?}", self.queue);
            let (hook, source_index) = self.queue.pop()?.data;

            if let Some(vector) = self.vectors.get_mut(source_index) {
                while let Some((next_hook, ordering)) = get_next_valid(hook_store, vector) {
                    if Some(next_hook) != self.last_hook {
                        self.queue.push(OrderAssociatedData {
                            data: (next_hook, source_index),
                            ordering: Reverse(ordering),
                        });
                        break;
                    }
                }
            } else {
                log_or_panic!(
                    "`source_index` {source_index} out of range of HookQueue vectors list (len {})",
                    self.vectors.len()
                );
            }

            // Skip duplicates
            if Some(hook) == self.last_hook {
                continue;
            }

            self.last_hook = Some(hook);
            return Some(hook);
        }
    }
}

/// Trait automatically implemented on reactive structs by the derive macro.
pub trait State: Sized + 'static {
    /// Overwrite the value of this state while preserving reactive tracking.
    /// Generally prefer dereferences.
    fn set(&mut self, new: Self);
}

impl State for () {
    fn set(&mut self, _new: Self) {}
}

impl<T: State> InnerCtx<T> {
    /// Loop over signals and update any depdant hooks for changed signals
    /// This also drains the deferred message queue
    fn update(&mut self, dep_lists: HookDepListHolder) {
        log::trace!("Performing update cycle for {}", std::any::type_name::<T>());

        log::trace!("{} signals changed", dep_lists.len());
        let mut hook_queue = HookQueue::new(&self.hooks, dep_lists);

        while let Some(hook_key) = hook_queue.pop(&self.hooks) {
            log::trace!("Updating hook {hook_key:?}");
            self.run_with_hook_and_self(hook_key, |ctx, hook| match hook.update(ctx, hook_key) {
                UpdateResult::RunHook(dep, drop) => {
                    hook_queue.push_next(dep);
                    for dep in drop {
                        ctx.hooks.drop_hook(dep);
                    }
                }
                UpdateResult::DropHooks(deps) => {
                    for dep in deps {
                        ctx.hooks.drop_hook(dep);
                    }
                }
            });
        }
        log::trace!("Update cycle complete");
    }

    /// Run the given method and track the reactive modifications done in it.
    /// and call initiate the update cycle afterwards.
    #[inline]
    pub(crate) fn track_changes<R>(&mut self, func: impl FnOnce(&mut Self) -> R) -> R {
        let (dirty_list, result) = statics::with_dirty_tracking(|| func(self));
        self.update(dirty_list);
        result
    }

    /// Run the given method and track reads, registering the given hook as a dependency of read
    /// signals
    #[inline]
    pub(crate) fn track_reads<R>(
        &mut self,
        hook: HookKey,
        func: impl for<'a> FnOnce(&'a mut Self) -> R,
    ) -> R {
        statics::with_hook(hook, || func(self))
    }
}
