//! Reactive system core - hook storage, scheduler, and global tracking state.
//!
//! This module contains the fundamental machinery that makes the reactivity system work:
//!
//! ## Hook Storage
//! - `HookStore`: Slotmap-based storage for reactive hooks
//! - `HookKey`: Type-safe keys with versioning to prevent use-after-free
//! - `SignalDepList`: Efficient linked list for tracking signal dependencies
//!
//! ## Scheduler
//! - `HookQueue`: Priority queue for processing hooks in topological order
//! - `ReactiveHook` trait: Interface all hooks must implement  
//! - `UpdateResult` and `RenderingState`: Types for coordinating updates
//!
//! ## Global Tracking (statics module)
//! - Thread-local state for tracking the current hook context
//! - Dirty list management for accumulating changed signals during updates

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::hash_map::Entry;

use smallvec::SmallVec;

use crate::error_handling::{log_or_panic, log_or_panic_assert};
use crate::reactivity::context::InnerCtx;
use crate::reactivity::{KeepAlive, State};

/// The slot of the key. This is the number of concurrent hooks we can have (around 65k).
type KeySlot = u16;

/// The version in a slot, used to detect stale keys.
/// This * `KeySlot` is the number of hooks we can have in the lifetime of the program
/// (including deallocated ones). Currently around 4 billion.
type KeyVersion = u16;

/// The type used to store the global insertion order.
/// Importantly this should have the same size as `HookKey`,
/// **but is NOT convertible between them**.
/// This is because we will never need more insertion orders than possible hooks,
/// but they are fundamentally different attributes of a key.
type InsertionOrder = u32;

/// A key into the hook slotmap
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct HookKey {
    /// The slot to use
    slot: KeySlot,
    /// Version used to avoid use-after-free
    version: KeyVersion,
}

impl HookKey {
    /// A fallback key for error paths
    fn fallback() -> Self {
        Self {
            slot: KeySlot::MAX,
            version: KeyVersion::MAX,
        }
    }

    /// Construct a new key from the given values, only used for testing from other modules.
    #[cfg(test)]
    pub(crate) fn new(slot: KeySlot, version: KeyVersion) -> Self {
        Self { slot, version }
    }
}

/// The value of a slot in the hook store
#[derive(Default)]
enum SlotValue<S> {
    /// The slot doesn't contain a value
    #[default]
    Empty,
    /// The slot is in use, but the value is moved out temporarily
    InUse,
    /// The slot is reserved (key allocated but hook not set yet)
    Reserved {
        /// The insertion order of the reserved slot
        order: InsertionOrder,
    },
    /// The slot is occupied with a hook
    Occupied {
        /// The hook implementation
        hook: Box<dyn ReactiveHook<S>>,
        /// The insertion order
        order: InsertionOrder,
    },
}

/// A slot in the hook slotmap
struct Slot<S> {
    /// The version of the slot
    version: KeyVersion,
    /// The value in the slot
    value: SlotValue<S>,
}

impl<S> Default for Slot<S> {
    fn default() -> Self {
        Self {
            version: 0,
            value: SlotValue::default(),
        }
    }
}

/// Storage for reactive hooks using a slotmap pattern
pub(super) struct HookStore<S: State> {
    /// The hooks themselves
    hooks: Vec<Slot<S>>,
    /// The free slots available for reuse
    free: Vec<KeySlot>,
    /// The next key in the insertion order
    next_insertion_order: InsertionOrder,
}

impl<S: State> HookStore<S> {
    /// Create a new hook store
    pub(super) fn new() -> Self {
        Self {
            hooks: Vec::with_capacity(100),
            free: Vec::with_capacity(10),
            next_insertion_order: 0,
        }
    }

    /// Reserve a hook key.
    ///
    /// INVARIANT: Hooks must call `.reserve_key` in the relative order they are required
    /// to be updated and invalidated.
    pub(super) fn reserve_key(&mut self) -> HookKey {
        let insertion_order = self.next_insertion_order;
        self.next_insertion_order = self.next_insertion_order.checked_add(1).unwrap_or_else(|| {
            log_or_panic!("Insertion order overflowed");
            // This is a very rare case, but restarting from zero should mean new hooks generally keep working.
            0
        });

        if let Some(slot) = self.free.pop() {
            let Some(entry) = self.hooks.get_mut(slot as usize) else {
                log_or_panic!("Value in free list out of bounds");
                return HookKey::fallback();
            };

            log_or_panic_assert!(
                matches!(entry.value, SlotValue::Empty),
                "Free slotmap wasn't `Empty`"
            );

            let Some(new_version) = entry.version.checked_add(1) else {
                log_or_panic!("Slot at max version was in free list");
                return self.reserve_key();
            };
            entry.version = new_version;

            entry.value = SlotValue::Reserved {
                order: insertion_order,
            };

            HookKey {
                slot,
                version: new_version,
            }
        } else {
            let Ok(slot) = self.hooks.len().try_into() else {
                log_or_panic!("Ran out of space in hooks slotmap");
                self.release_fallback_reclaim_high_versions();
                return self.reserve_key();
            };

            self.hooks.push(Slot {
                version: 0,
                value: SlotValue::Reserved {
                    order: insertion_order,
                },
            });
            HookKey { slot, version: 0 }
        }
    }

    /// Marks all `Empty` slots as free even if their version number is at `MAX`.
    /// This case is extremely rare, and this recovery path is good enough in most cases.
    fn release_fallback_reclaim_high_versions(&mut self) {
        for (index, slot) in self.hooks.iter_mut().enumerate() {
            if matches!(slot.value, SlotValue::Empty) {
                slot.version = 0;
                if let Ok(index) = index.try_into() {
                    self.free.push(index);
                } else {
                    log_or_panic!("Vec index overflows slot");
                }
            }
        }
    }

    /// Update the value for a hook
    pub(super) fn set_hook(&mut self, key: HookKey, hook: Box<dyn ReactiveHook<S>>) {
        if let Some(slot) = self.hooks.get_mut(key.slot as usize) {
            // `set_hook` is always used directly after hook creation.
            log_or_panic_assert!(
                key.version == slot.version,
                "Mismatched version between key and slot in `set_hook`"
            );

            if let SlotValue::Reserved { order } = slot.value {
                slot.value = SlotValue::Occupied { hook, order };
            } else {
                log_or_panic!("Target slot wasn't reserved");
            }
        } else {
            log_or_panic!("Attempted to update missing slot {}", key.slot);
        }
    }

    /// Drop the hook and all of its children
    fn drop_hook(&mut self, target_hook_key: HookKey) {
        let mut hooks_to_drop = vec![target_hook_key];

        while let Some(hook_key) = hooks_to_drop.pop() {
            if let Some(slot) = self.hooks.get_mut(hook_key.slot as usize) {
                if slot.version != hook_key.version {
                    continue;
                }

                self.free.push(hook_key.slot);
                match std::mem::take(&mut slot.value) {
                    SlotValue::Empty | SlotValue::InUse => {}
                    SlotValue::Reserved { .. } => {
                        log_or_panic!("Attempted to drop reserved hook.");
                    }
                    SlotValue::Occupied { hook, .. } => {
                        hooks_to_drop.extend(hook.drop_us());
                    }
                }
            } else {
                log_or_panic!("Attempted to drop hook outside current allocated index.");
            }
        }
    }

    /// Get the insertion order at a given hook, returns `None` if hook doesn't exist.
    fn get_insertion_order(&self, key: HookKey) -> Option<InsertionOrder> {
        if let Some(slot) = self.hooks.get(key.slot as usize) {
            if slot.version != key.version {
                return None;
            }

            match slot.value {
                SlotValue::Empty | SlotValue::InUse => None,
                SlotValue::Occupied { order, .. } | SlotValue::Reserved { order } => Some(order),
            }
        } else {
            log_or_panic!("hook key out of bounds.");
            None
        }
    }
}

impl<S: State> InnerCtx<S> {
    /// Remove the hook from the slotmap, run the function on it, then put it back.
    ///
    /// This is to allow mutable access to both the hook and self, which is required by most hooks
    /// (hooks often mutably access the slotmap while running).
    fn run_with_hook_and_self<F, R>(&mut self, hook_key: HookKey, func: F) -> Option<R>
    where
        F: FnOnce(&mut Self, &mut Box<dyn ReactiveHook<S>>) -> R,
    {
        let Some(before_slot_ref) = self.hooks.hooks.get_mut(hook_key.slot as usize) else {
            log_or_panic!("HookKey outside bounds of slotmap");
            return None;
        };
        if before_slot_ref.version != hook_key.version {
            log::trace!("Version mismatch in `run_with_hook_and_self`");
            return None;
        }

        let mut slot_value = SlotValue::InUse;
        std::mem::swap(&mut slot_value, &mut before_slot_ref.value);

        let (order, mut hook) = match slot_value {
            SlotValue::Empty => {
                return None;
            }
            SlotValue::InUse => {
                log_or_panic!("Re-entry in `run_with_hook_and_self`");
                return None;
            }
            SlotValue::Reserved { .. } => {
                log_or_panic!("`run_with_hook_and_self` hit reserved hook");
                return None;
            }
            SlotValue::Occupied { hook, order } => (order, hook),
        };

        let res = func(self, &mut hook);

        let Some(after_slot_ref) = self.hooks.hooks.get_mut(hook_key.slot as usize) else {
            log_or_panic!("HookKey outside bounds of slotmap");
            return None;
        };

        if matches!(after_slot_ref.value, SlotValue::InUse) {
            after_slot_ref.value = SlotValue::Occupied { hook, order };
        } else {
            log_or_panic_assert!(
                matches!(after_slot_ref.value, SlotValue::Empty),
                "Slotmap entry overwritten in `run_with_hook_and_self`"
            );
        }

        Some(res)
    }
}

/// A linked list for holding signal dependencies.
/// Allows O(1) move to end and deduplication,
/// while removing stale entries based on slotmap.
///
/// Worst case this is bound to the size of the max amount of concurrent hooks.
/// More likely it will efficiently re-use memory even if rarely drained.
pub(super) struct SignalDepList {
    /// The allocations of the nodes themselves
    items: nohash::IntMap<KeySlot, SignalDepNode>,
    /// The start of the list
    head: Option<KeySlot>,
    /// The end of the list
    tail: Option<KeySlot>,
}

/// A node in the linked list
struct SignalDepNode {
    /// The actual full hook key version
    version: KeyVersion,
    /// The index of the previous node
    previous: Option<KeySlot>,
    /// The index of the next node
    next: Option<KeySlot>,
}

impl Default for SignalDepList {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalDepList {
    /// Create a new empty signal dep list
    pub(super) fn new() -> Self {
        Self {
            items: nohash::IntMap::default(),
            head: None,
            tail: None,
        }
    }

    /// Get the amount of current hooks (including stale ones.)
    #[cfg(test)]
    fn len(&self) -> usize {
        self.items.len()
    }

    /// Insert a new key into the linked list.
    /// This re-uses nodes with matching slots, as well as ensures proper ordering.
    pub(super) fn insert(&mut self, key: HookKey) {
        match self.items.entry(key.slot) {
            Entry::Vacant(entry) => {
                let node = SignalDepNode {
                    version: key.version,
                    previous: self.tail,
                    next: None,
                };
                entry.insert(node);

                if let Some(tail_slot) = self.tail.replace(key.slot) {
                    if let Some(tail) = self.items.get_mut(&tail_slot) {
                        tail.next = Some(key.slot);
                    } else {
                        log_or_panic!("Tail for signal dep list not found");
                    }
                }

                if self.head.is_none() {
                    self.head = Some(key.slot);
                }
            }
            Entry::Occupied(mut entry) => {
                let node = entry.get_mut();

                // No need to move to end as there is no change
                // then this should already be in the correct relative position.
                if node.version == key.version {
                    return;
                }
                node.version = key.version;

                // if `next` is `None` we are tail.
                if let Some(next_slot) = node.next.take() {
                    let tail = self.tail.replace(key.slot);
                    let previous = std::mem::replace(&mut node.previous, tail);
                    if let Some(tail) = tail {
                        let Some(tail) = self.items.get_mut(&tail) else {
                            log_or_panic!("Next not found");
                            return;
                        };
                        tail.next = Some(key.slot);
                    }

                    match previous {
                        // We are head
                        None => {
                            let Some(next) = self.items.get_mut(&next_slot) else {
                                log_or_panic!("Next not found");
                                return;
                            };
                            next.previous = None;
                            self.head = Some(next_slot);
                        }
                        // We are somewhere else
                        Some(previous_slot) => {
                            let Some(next) = self.items.get_mut(&next_slot) else {
                                log_or_panic!("Next not found");
                                return;
                            };
                            next.previous = Some(previous_slot);

                            let Some(previous_slot) = self.items.get_mut(&previous_slot) else {
                                log_or_panic!("Previous not found");
                                return;
                            };
                            previous_slot.next = Some(next_slot);
                        }
                    }
                }
            }
        }
    }

    /// Create an iterator over the current nodes by moving them in,
    /// and clear the leftover metadata.
    /// This moves the map into the iterator and allocates a new map with the same capacity
    /// (since it's likely we will get close to the same amount of signals.)
    pub(super) fn create_iter_and_clear(&mut self) -> IterSignalList {
        let new_map = nohash::IntMap::with_capacity_and_hasher(
            self.items.len(),
            nohash::BuildNoHashHasher::new(),
        );
        let iterator = IterSignalList {
            nodes: std::mem::replace(&mut self.items, new_map),
            next: self.head,
        };
        self.head = None;
        self.tail = None;
        iterator
    }
}

/// An iterator over the `HookKey`s in a `SignalDepList`
pub(super) struct IterSignalList {
    /// The linked list nodes
    nodes: nohash::IntMap<KeySlot, SignalDepNode>,
    /// The index of the next node
    next: Option<KeySlot>,
}

impl Iterator for IterSignalList {
    type Item = HookKey;

    fn next(&mut self) -> Option<Self::Item> {
        let slot = self.next?;
        if let Some(next) = self.nodes.get(&slot) {
            self.next = next.next;
            Some(HookKey {
                slot,
                version: next.version,
            })
        } else {
            log_or_panic!("Next item not found in signal iterator.");
            None
        }
    }
}

/// The type that will be used to hold the dirty lists.
type HookDepListHolder = SmallVec<[IterSignalList; 2]>;

/// Thread-local state for tracking the current hook and dirty signals.
pub(super) mod statics {
    use std::cell::{Cell, RefCell};

    use smallvec::SmallVec;

    use super::{HookDepListHolder, HookKey, IterSignalList};
    use crate::error_handling::{log_or_panic, log_or_panic_assert};

    thread_local! {
        /// The current hook the signal is being accessed in.
        static CURRENT_HOOK: Cell<Option<HookKey>> = const { Cell::new(None) };
        /// List for signals to push deps lists into
        static DIRTY_HOOKS: RefCell<Option<HookDepListHolder>> = const { RefCell::new(None) };
    }

    /// Return the current hook if any
    #[inline]
    pub(in crate::reactivity) fn current_hook() -> Option<HookKey> {
        CURRENT_HOOK.get()
    }

    /// Run the given function with the given hook as the current hook
    /// and restore the previous hook on completion.
    #[inline]
    pub(in crate::reactivity) fn with_hook<R>(new_hook: HookKey, func: impl FnOnce() -> R) -> R {
        let previous_hook = CURRENT_HOOK.replace(Some(new_hook));
        let result = func();
        CURRENT_HOOK.set(previous_hook);
        result
    }

    /// Push an iterator to the dirty hooks list
    #[inline]
    pub(in crate::reactivity) fn reg_dirty_list(calc: impl FnOnce() -> IterSignalList) {
        DIRTY_HOOKS.with(|dirty_hooks| {
            let Ok(mut dirty_hooks) = dirty_hooks.try_borrow_mut() else {
                log_or_panic!("`DIRTY_HOOKS` overlapping borrow");
                return;
            };

            if let Some(dirty_hooks) = &mut *dirty_hooks {
                dirty_hooks.push(calc());
            }
        });
    }

    /// Drain the list of dirty hooks
    #[inline]
    pub(in crate::reactivity) fn with_dirty_tracking<R>(
        func: impl FnOnce() -> R,
    ) -> (HookDepListHolder, R) {
        DIRTY_HOOKS.with(|dirty_hooks| {
            let Ok(mut dirty_hooks) = dirty_hooks.try_borrow_mut() else {
                log_or_panic!("`DIRTY_HOOKS` overlapping borrow");
                return;
            };

            log_or_panic_assert!(
                dirty_hooks.is_none(),
                "`with_dirty_tracking` called recursively"
            );
            *dirty_hooks = Some(SmallVec::new());
        });

        let result = func();

        let dirty_list = DIRTY_HOOKS.with(|dirty_hooks| {
            let Ok(mut dirty_hooks) = dirty_hooks.try_borrow_mut() else {
                log_or_panic!("`DIRTY_HOOKS` overlapping borrow");
                return SmallVec::new();
            };

            let Some(dirty_hooks) = dirty_hooks.take() else {
                log_or_panic!("`DIRTY_HOOKS` gone after being set");
                return SmallVec::new();
            };

            dirty_hooks
        });

        (dirty_list, result)
    }

    /// Clear the statics
    #[cfg(feature = "test_utils")]
    pub fn clear() {
        DIRTY_HOOKS.set(None);
        CURRENT_HOOK.set(None);
    }
}

// Re-export statics functions for use within reactivity module
#[cfg(feature = "test_utils")]
pub(crate) use statics::clear;

/// State passed to rendering callbacks and hooks
pub(crate) struct RenderingState<'state> {
    /// Push objects to this array to keep them alive as long as the parent context is valid.
    pub(crate) keep_alive: &'state mut Vec<KeepAlive>,
    /// The hooks that are a child of this
    pub(crate) hooks: &'state mut Vec<HookKey>,
}

/// The result of a hook update
pub(super) enum UpdateResult {
    /// Drop the given hooks
    DropHooks(Vec<HookKey>),
    /// Run this hook after this one
    ///
    /// INVARIANT: This can only be returned if the only reason you are running is because said
    /// hook could also be run (i.e you are acting as a pre-hook check, such as with `.watch`).
    /// As this hook is run instantly, if this is used to run a hook before a parent may have
    /// invalidated it, that might lead to panics if said hook uses features such as Guards.
    RunHook(HookKey, Vec<HookKey>),
}

/// All reactive hooks implement this trait to allow them to be stored as `dyn` objects.
pub(super) trait ReactiveHook<S: State> {
    /// Recalculate the hook and apply its update.
    ///
    /// Hooks should re-register dependencies by calling this with the `you` parameter,
    /// since dependencies are drained during update to ensure conditional reactive state
    /// is properly tracked.
    fn update(&mut self, ctx: &mut InnerCtx<S>, you: HookKey) -> UpdateResult;

    /// Return the list of child hooks that should be dropped when this hook is dropped
    fn drop_us(self: Box<Self>) -> Vec<HookKey>;
}

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

/// The queue processor for the update cycle
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

/// Iterate over `iter` until it finds a key with a valid insertion order
fn get_next_valid<S: State>(
    hook_store: &HookStore<S>,
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
    fn new<S: State>(hook_store: &HookStore<S>, mut vectors: HookDepListHolder) -> Self {
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

    /// Push an item to be popped next
    fn push_next(&mut self, key: HookKey) {
        if self.next_item.is_some() {
            log_or_panic!("`push_next` called while `next_item` already has item");
        }

        self.next_item = Some(key);
    }

    /// Pop the next item
    fn pop<S: State>(&mut self, hook_store: &HookStore<S>) -> Option<HookKey> {
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

impl<S: State> InnerCtx<S> {
    /// Process all changed signals and update any dependent hooks.
    /// Hooks are run in topological order based on insertion order.
    fn update(&mut self, dep_lists: HookDepListHolder) {
        log::trace!("Performing update cycle for {}", std::any::type_name::<S>());

        log::trace!("{} signals changed", dep_lists.len());
        let mut hook_queue = HookQueue::new(&self.hooks, dep_lists);

        while let Some(hook_key) = hook_queue.pop(&self.hooks) {
            log::trace!("Updating hook {hook_key:?}");
            self.run_with_hook_and_self(hook_key, |ctx, hook| match hook.update(ctx, hook_key) {
                UpdateResult::RunHook(dep, drop) => {
                    hook_queue.push_next(dep);
                    for to_drop in drop {
                        ctx.hooks.drop_hook(to_drop);
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

    /// Run the given method, track the reactive modifications done in it,
    /// and initiate the update cycle afterwards.
    #[inline]
    pub(crate) fn track_changes<R>(&mut self, func: impl FnOnce(&mut Self) -> R) -> R {
        let (dirty_list, result) = statics::with_dirty_tracking(|| func(self));
        self.update(dirty_list);
        result
    }

    /// Run the given method and track reads, registering the given hook as a dependency
    /// of read signals.
    #[inline]
    pub(super) fn track_reads<R>(
        &mut self,
        hook: HookKey,
        func: impl for<'state> FnOnce(&'state mut Self) -> R,
    ) -> R {
        statics::with_hook(hook, || func(self))
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
#[expect(clippy::expect_used, clippy::unreachable, reason = "tests")]
mod tests {
    use proptest::proptest;

    use super::*;

    fn key(slot: KeySlot, version: KeyVersion) -> HookKey {
        HookKey { slot, version }
    }

    #[test]
    fn insertion_order_kept_for_unique_slots() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(1, 0));
        list.insert(key(2, 0));
        list.insert(key(3, 0));
        list.insert(key(4, 0));
        list.insert(key(5, 0));

        let slots = list
            .create_iter_and_clear()
            .map(|key| key.slot)
            .collect::<Vec<_>>();
        assert_eq!(slots, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn creating_iterator_clears_list() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(1, 0));
        list.insert(key(2, 0));
        list.insert(key(3, 0));
        list.insert(key(4, 0));
        list.insert(key(5, 0));
        list.create_iter_and_clear();

        assert_eq!(list.len(), 0);
    }

    #[test]
    fn can_iter_empty_list() {
        let mut list = SignalDepList::new();
        let mut iter = list.create_iter_and_clear();

        assert_eq!(iter.next(), None);
    }

    #[test]
    fn can_iter_one_element_list() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));

        let mut iter = list.create_iter_and_clear();
        assert_eq!(iter.next(), Some(key(0, 0)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reuse_slot_single() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(0, 1));

        assert_eq!(list.len(), 1);
        let mut iter = list.create_iter_and_clear();
        assert_eq!(iter.next(), Some(key(0, 1)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reuse_slot_head() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(1, 0));
        list.insert(key(2, 0));

        list.insert(key(0, 1));
        assert_eq!(list.len(), 3);
        let mut iter = list.create_iter_and_clear();
        assert_eq!(iter.next(), Some(key(1, 0)));
        assert_eq!(iter.next(), Some(key(2, 0)));
        assert_eq!(iter.next(), Some(key(0, 1)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reuse_slot_tail() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(1, 0));
        list.insert(key(2, 0));

        list.insert(key(2, 1));
        assert_eq!(list.len(), 3);
        let mut iter = list.create_iter_and_clear();
        assert_eq!(iter.next(), Some(key(0, 0)));
        assert_eq!(iter.next(), Some(key(1, 0)));
        assert_eq!(iter.next(), Some(key(2, 1)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reuse_slot_middle() {
        let mut list = SignalDepList::new();
        list.insert(key(0, 0));
        list.insert(key(1, 0));
        list.insert(key(2, 0));

        list.insert(key(1, 1));
        assert_eq!(list.len(), 3);
        let mut iter = list.create_iter_and_clear();
        assert_eq!(iter.next(), Some(key(0, 0)));
        assert_eq!(iter.next(), Some(key(2, 0)));
        assert_eq!(iter.next(), Some(key(1, 1)));
        assert_eq!(iter.next(), None);
    }

    proptest! {
        #[test]
        fn linked_list_doesnt_panic(slots: Vec<(KeySlot, KeyVersion)>) {
            let mut list = SignalDepList::new();
            for (slot,version) in slots {
                list.insert(key(slot, version));
            }
            for _ in list.create_iter_and_clear() {}
        }

        #[test]
        fn iter_length_eq_list_length(slots: Vec<(KeySlot, KeyVersion)>) {
            let mut list = SignalDepList::new();
            for (slot,version) in slots {
                list.insert(key(slot, version));
            }

            let list_length = list.len();
            let iter_length = list.create_iter_and_clear().count();
            assert_eq!(list_length, iter_length);
        }
    }

    #[test]
    fn outside_closure_is_no_hook() {
        let hook1 = key(0, 0);

        assert_eq!(statics::current_hook(), None);
        statics::with_hook(hook1, || {});
        assert_eq!(statics::current_hook(), None);
    }

    #[test]
    fn setting_hook_gives_hook() {
        let hook1 = key(0, 0);
        statics::with_hook(hook1, || {
            assert_eq!(statics::current_hook(), Some(hook1));
        });
    }

    #[test]
    fn nesting_hook() {
        let hook1 = key(0, 0);
        let hook2 = key(1, 0);

        statics::with_hook(hook1, || {
            statics::with_hook(hook2, || {
                assert_eq!(statics::current_hook(), Some(hook2));
            });
            assert_eq!(statics::current_hook(), Some(hook1));
        });
    }

    #[test]
    fn dirty_tracking() {
        let hook1 = key(0, 0);
        let hook2 = key(1, 0);

        let (mut result, ()) = statics::with_dirty_tracking(|| {
            statics::reg_dirty_list(|| {
                let mut keys = SignalDepList::new();
                keys.insert(hook1);
                keys.insert(hook2);
                keys.create_iter_and_clear()
            });
        });

        assert_eq!(result.len(), 1);
        let iter = result.first_mut().expect("No results in dirty tracking.");
        assert_eq!(iter.next(), Some(hook1));
        assert_eq!(iter.next(), Some(hook2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reg_dirty_list_lazy() {
        statics::reg_dirty_list(|| {
            unreachable!("Dirty list closure was called even though no dirty tracking active.")
        });
    }
}
