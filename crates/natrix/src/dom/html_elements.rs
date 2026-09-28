//! Implementation of html elements, as well as helper constructors.
//!
//! This module is generally used via its alias in the prelude, `e`.
//! Most commonly you will just use the element functions directly, but you can construct a
//! `HtmlElement` instance if needed.
//!
//! # Example
//! ```no_run
//! # use natrix::prelude::*;
//! # let _: e::HtmlElement<(), _> =
//! e::div()
//!     .child(e::button().text("Click me"))
//!     .child(e::h1().text("Wow!"))
//! # ;
//! ```

#[cfg(debug_assertions)]
use std::collections::HashSet;
use std::marker::PhantomData;
use std::rc::Weak;

use smallvec::SmallVec;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, intern};

use super::attributes::AttributeResult;
use super::classes::ClassResult;
use crate::css::selectors::{CompoundSelector, IntoCompoundSelector, SimpleSelector};
use crate::dom::element::{Element, MaybeStaticElement, generate_fallback_node};
use crate::dom::events::{Event, EventHandler};
use crate::dom::{ToAttribute, ToClass, attributes};
use crate::error_handling::{log_or_panic, log_or_panic_result};
use crate::prelude::Id;
use crate::reactivity::State;
use crate::reactivity::context::InnerCtx;
use crate::reactivity::core::RenderingState;
use crate::{EventCtx, get_document};

/// A deferred function to do something once state is available
pub(crate) type DeferredFunc<C> = Box<dyn FnOnce(&mut InnerCtx<C>, &mut RenderingState)>;

/// Indicates the given element is allowed to children
/// This will catch errors such as:
/// ```compile_fail
/// # use natrix::prelude::*;
/// # let _: e::HtmlElement<(), _> =
/// e::br().child(e::h1().text("huh"))
/// # ;
/// ```
pub trait CanHaveChild {}

impl CanHaveChild for () {}

/// A Generic html node with a given name.
#[must_use = "Web elements are useless if not rendered"]
#[non_exhaustive]
pub struct HtmlElement<C: State, T = ()> {
    /// Dom element
    element: web_sys::Element,
    /// The deferred actions
    deferred: SmallVec<[DeferredFunc<C>; 2]>,
    /// Phantom data
    _phantom: PhantomData<T>,
    /// List of attributes that are already set.
    #[cfg(debug_assertions)]
    seen_attributes: HashSet<&'static str>,
    /// List of set of reactive attributes.
    #[cfg(debug_assertions)]
    reactive_attributes: HashSet<&'static str>,
}

impl<C: State, T> HtmlElement<C, T> {
    /// Create a new html element with the specific tag
    ///
    /// All non-deprecated html elements have a helper function in this module
    pub fn new(tag: &'static str) -> Self {
        let node = if let Ok(node) = get_document().create_element(intern(tag)) {
            node
        } else {
            log_or_panic!("Failed to create <{tag}>");
            generate_fallback_node().unchecked_into()
        };

        Self {
            element: node,
            deferred: SmallVec::new(),
            _phantom: PhantomData,
            #[cfg(debug_assertions)]
            seen_attributes: HashSet::new(),
            #[cfg(debug_assertions)]
            reactive_attributes: HashSet::new(),
        }
    }

    /// Returns the raw html element behind this `HtmlElement`
    /// This is intended for performing modifications natrix does not natively expose.
    ///
    /// WARN: reactive callbacks (`|ctx| ...`) have not been called at this point as such the
    /// attributes and child lists of the given element might not match its final state.
    #[must_use]
    pub fn get_element(&self) -> &web_sys::Element {
        &self.element
    }

    /// Drain and return the deferred functions for this html element
    pub(crate) fn drain_deferred(&mut self) -> impl Iterator<Item = DeferredFunc<C>> {
        self.deferred.drain(..)
    }

    /// Replace the tag type marker with `()` to allow returning different types of elements.
    /// This should mainly be used when you want to call additional builder methods on the generic
    /// builder.
    /// If you want to return different element types from a closure you can use `.render` instead.
    pub fn generic(self) -> HtmlElement<C, ()> {
        HtmlElement {
            element: self.element,
            deferred: self.deferred,
            #[cfg(debug_assertions)]
            seen_attributes: self.seen_attributes,
            #[cfg(debug_assertions)]
            reactive_attributes: self.reactive_attributes,
            _phantom: PhantomData,
        }
    }

    /// Register a event handler for this element.
    ///
    /// The event handler is a closure taking a mutable reference to `S<Self>`.
    /// ```rust
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct MyState {
    /// #     some_value: Signal<i32>,
    /// # }
    /// # fn render() -> impl Element<MyState> {
    /// e::button().on::<events::Click>(|mut ctx: EventCtx<MyState>, _| {
    ///     *ctx.some_value += 1;
    /// })
    /// # }
    /// ```
    /// For more information see [Reactivity](https://serpent-tools.github.io/natrix/reactivity.html) in the book.
    #[inline]
    pub fn on<E: Event>(mut self, function: impl EventHandler<C, E>) -> Self {
        let function = function.func();
        let element: &web_sys::Element = self.element.as_ref();
        let element = element.clone();

        self.deferred.push(Box::new(move |ctx, rendering_state| {
            let ctx_weak = Weak::clone(&ctx.this);

            let function: Box<dyn Fn(web_sys::Event) + 'static> = Box::new(move |event| {
                crate::panics::return_if_panic!();

                let Ok(event) = event.dyn_into() else {
                    log_or_panic!("Unexpected event type");
                    return;
                };

                let Some(event_ctx) = ctx_weak.upgrade() else {
                    log_or_panic!("State dropped without event handlers being cleaned up");
                    return;
                };
                let Ok(mut event_ctx) = event_ctx.try_borrow_mut() else {
                    log_or_panic!("State already mutably borrowed in event handler");
                    return;
                };

                event_ctx.track_changes(|event_ctx| {
                    function(EventCtx(event_ctx), event);
                });
            });
            let closure = Closure::wrap(function);
            let js_function = closure.as_ref().unchecked_ref();

            log_or_panic_result!(
                element.add_event_listener_with_callback(intern(E::EVENT_NAME), js_function),
                "Failed to attach event handler"
            );

            rendering_state.keep_alive.push(Box::new(closure));
        }));
        self
    }

    /// Push a child to this element.
    /// This accepts any valid element including closures.
    /// ```rust
    /// # use natrix::prelude::*;
    /// # #[derive(State)]
    /// # struct MyState {
    /// #     toggle: Signal<bool>,
    /// # }
    /// # fn render() -> impl Element<MyState> {
    /// e::div()
    ///     .child(e::h1().text("Wow!"))
    ///     .child(|ctx: RenderCtx<MyState>| {
    ///         if *ctx.toggle {
    ///             "Hello"
    ///         } else {
    ///             "World"
    ///         }
    ///     })
    /// # }
    /// ```
    #[inline]
    pub fn child<E: Element<C> + 'static>(mut self, child: E) -> Self
    where
        T: CanHaveChild,
    {
        let node = match child.render() {
            MaybeStaticElement::Static(result) => result.into_node(),
            MaybeStaticElement::Html(html) => {
                self.deferred.extend(html.deferred);
                html.element.into()
            }
            MaybeStaticElement::Dynamic(dynamic) => {
                let Ok(comment) = web_sys::Comment::new() else {
                    log_or_panic!("Failed to create placeholder comment node");
                    return self;
                };
                let comment_clone = comment.clone();
                self.deferred.push(Box::new(move |ctx, rendering_state| {
                    let node = dynamic.render(ctx, rendering_state).into_node();
                    log_or_panic_result!(
                        comment_clone.replace_with_with_node_1(&node),
                        "Failed to swap in child"
                    );
                }));
                comment.into()
            }
        };

        let element: &web_sys::Element = self.element.as_ref();
        log_or_panic_result!(element.append_child(&node), "Failed to append child");

        self
    }

    /// This is a simple alias for `child`
    #[inline]
    pub fn text<E: Element<C> + 'static>(self, text: E) -> Self
    where
        T: CanHaveChild,
    {
        self.child(text)
    }

    /// Set a attribute on the node.
    #[inline]
    pub fn attr(mut self, key: &'static str, value: impl ToAttribute<C>) -> Self {
        #[cfg(debug_assertions)]
        {
            if self.seen_attributes.contains(key) {
                log::warn!(
                    "Duplicate `{key}` attribute set on `<{}>`",
                    self.element.tag_name(),
                );
            }
            self.seen_attributes.insert(key);
        }

        match value.calc_attribute(intern(key), &self.element) {
            AttributeResult::SetIt(res) => {
                if let Some(res) = res {
                    log_or_panic_result!(
                        self.element.set_attribute(key, &res),
                        "Failed to set attribute"
                    );
                }
            }
            AttributeResult::IsDynamic(dynamic) => {
                #[cfg(debug_assertions)]
                {
                    if self.reactive_attributes.contains(key) {
                        log_or_panic!(
                            "Multiple reactive closures set on attribute `{key}`, this would cause un-deterministic state. (on a `<{}>` tag)",
                            self.element.tag_name()
                        );
                    }
                    self.reactive_attributes.insert(key);
                }

                self.deferred.push(Box::new(dynamic));
            }
        }

        self
    }

    /// Add a class to the element.
    #[inline]
    pub fn class(mut self, class: impl ToClass<C> + 'static) -> Self {
        match class.calc_class(&self.element) {
            ClassResult::SetIt(res) => {
                if let Some(res) = res {
                    log_or_panic_result!(
                        self.element.class_list().add_1(intern(&res)),
                        "Failed to add class"
                    );
                }
            }
            ClassResult::Dynamic(dynamic) => {
                self.deferred.push(Box::new(dynamic));
            }
        }

        self
    }

    /// Add multiple classes
    #[inline]
    pub fn classes<Cls: ToClass<C> + 'static>(
        mut self,
        class_list: impl IntoIterator<Item = Cls>,
    ) -> Self {
        for class in class_list {
            self = self.class(class);
        }
        self
    }

    /// Add multiple children, this is most commonly used with the `format_elements!` macro
    #[inline]
    pub fn children<E: Element<C>>(mut self, elements: impl IntoIterator<Item = E>) -> Self
    where
        T: CanHaveChild,
    {
        for element in elements {
            self = self.child(element);
        }
        self
    }
}

impl<C: State, T: 'static> Element<C> for HtmlElement<C, T> {
    #[inline]
    fn render(self) -> MaybeStaticElement<C> {
        MaybeStaticElement::Html(self.generic())
    }
}

/// Implement a factory function that returns a `HtmlElement` with a tag name equal to the
/// function.
macro_rules! elements {
    ($($name:ident),*) => {
        pastey::paste! {
            $(
                #[doc = "<https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/" $name ">"]
                pub struct [< Tag $name:camel >];

                impl IntoCompoundSelector for [< Tag $name:camel >] {
                    fn into_compound(self) -> CompoundSelector {
                        CompoundSelector(vec![SimpleSelector::Tag(stringify!($name).into())])
                    }
                }

                #[doc = "<https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/" $name ">"]
                #[inline]
                #[expect(clippy::allow_attributes, reason = "not all macro calls trigger it")]
                #[allow(clippy::min_ident_chars, reason = "html tags")]
                pub fn $name<C: State>() -> HtmlElement<C, [< Tag $name:camel >]> {
                    HtmlElement::new(stringify!($name))
                }
            )*
        }
    };
}

/// Mark that the list of elements can have children
macro_rules! can_have_children {
    ($($tag:ident),*) => {
        pastey::paste! {
            $(
                impl CanHaveChild for [< Tag $tag:camel >] {}
            )*
        }
    };
}

/// A macro to define `attr` helpers for the the various elements
macro_rules! attr_helpers {
    ($tag:ident => $($attr:ident($kind:path, $attr_name:literal $(, $alias:literal)?)),+) => {
        pastey::paste! {
            impl<C: State> HtmlElement<C, [< Tag $tag:camel >]> {
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/" $tag "##" $attr_name ">"]
                    $(#[doc(alias = $alias)])?
                    #[inline]
                    pub fn $attr(self, value: impl ToAttribute<C, AttributeKind = $kind>) -> Self {
                        self.attr($attr_name, value)
                       }
                )+
            }
        }
    };
}

/// Generate a `attr` helpers implementation for the global attributes
macro_rules! global_attrs {
    ($($attr:ident($kind:path, $attr_value:literal)),*) => {
        pastey::paste! {
            impl<C: State, T> HtmlElement<C, T> {
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/HTML/Reference/Global_attributes/" $attr_value ">"]
                    #[inline]
                    pub fn $attr(self, value: impl ToAttribute<C, AttributeKind=$kind>) -> Self {
                        self.attr($attr_value, value)
                    }
                )*
            }
        }
    };
}

/// Generate a `attr` helpers implementation for the aria attributes
macro_rules! aria_attrs {
    ($($attr:ident),*) => {
        impl<C: State, T> HtmlElement<C, T> {
            pastey::paste! {
                $(
                    #[doc = "<https://developer.mozilla.org/docs/Web/Accessibility/ARIA/Reference/Attributes/aria%2d" $attr ">"]
                    #[inline]
                    pub fn [<aria_$attr>](self, value: impl ToAttribute<C>) -> Self {
                        self.attr(concat!("aria-", stringify!($attr)), value)
                    }
                )*
            }
        }
    };
}

// NOTE:
// "sane defaults" should not be the defaults for element constructors.
// and should instead be implemented via extra methods, such as `.secure`

// https://developer.mozilla.org/en-US/docs/Web/HTML/Element
elements! {
    h1, h2, h3, h4, h5, h6,
    address, article, aside, footer, header, hgroup, main, nav, section, search,
    blockquote, dd, div, dl, dt, figcaption, figure, hr, li, menu, ol, p, pre, ul,
    a, abbr, b, bdi, bdo, br, cite, code, data, dfn, em, i, kbd, mark, q, rp, rt, ruby, s, samp, small, span, strong, sub, sup, time, u, var, wbr,
    area, audio, img, map, track, video,
    embed, fencedframe, iframe, object, picture, source,
    svg, math,
    canvas, script,
    del, ins,
    caption, col, colgroup, table, tbody, td, tfoot, th, thead, tr,
    button, datalist, fieldset, form, input, label, legend, meter, optgroup, option, output, progress, select, textarea,
    details, dialog, summary
}

// https://developer.mozilla.org/en-US/docs/Glossary/Void_element
// NOTE: We cant have negative trait bounds, so we list out all the elements instead.
can_have_children! {
    h1, h2, h3, h4, h5, h6,
    address, article, aside, footer, header, hgroup, main, nav, section, search,
    blockquote, dd, div, dl, dt, figcaption, figure, li, menu, ol, p, pre, ul,
    a, abbr, b, bdi, bdo, cite, code, data, dfn, em, i, kbd, mark, q, rp, rt, ruby, s, samp, small, span, strong, sub, sup, time, u, var,
    audio, map, video,
    fencedframe, iframe, object, picture,
    svg, math,
    canvas, script,
    del, ins,
    caption, colgroup, table, tbody, td, tfoot, th, thead, tr,
    button, datalist, fieldset, form, label, legend, meter, optgroup, option, output, progress, select, textarea,
    details, dialog, summary
}

// https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes
global_attrs! {
    access_key(char, "accesskey"), auto_focus(bool, "autofocus"), content_editable(attributes::ContentEditable, "contenteditable"),
    dir(attributes::Direction, "dir"), draggable(attributes::TrueFalse, "draggable"), enter_key_hint(attributes::EnterkeyHint, "enterkeyhint"),
    hidden(bool, "hidden"), id(Id, "id"), inert(bool, "inert"), input_mode(attributes::InputMode, "inputmode"), lang(String, "lang"),
    popover(attributes::PopOver, "popover"), spellcheck(bool, "spellcheck"), tab_index(attributes::Integer, "tabindex"),
    title(String, "title"), translate(attributes::YesNo, "translate"), auto_capitalize(attributes::AutoCapitalize, "autocapitalize")
}

aria_attrs! {
    autocomplete,
    checked,
    disabled,
    errormessage,
    expanded,
    haspopup,
    hidden,
    invalid,
    label,
    level,
    modal,
    multiline,
    multiselectable,
    orientation,
    placeholder,
    pressed,
    readonly,
    required,
    selected,
    sort,
    valuemax,
    valuemin,
    valuenow,
    valuetext
}

attr_helpers!(a =>
    download(bool, "download"), href(String, "href"), href_lang(String, "hreflang"),
    ping(String, "ping"), referrer_policy(attributes::ReferrerPolicy, "referrerpolicy"), rel(attributes::Rel, "rel"),
    target(attributes::Target, "target")
);

impl<C: State> HtmlElement<C, TagA> {
    /// add `target="_blank"`
    #[inline]
    pub fn open_in_new_tab(self) -> Self {
        self.target(attributes::Target::NewTab)
    }

    /// add `rel="noopener noreferrer" referrerpolicy="no-referrer"`
    #[inline]
    pub fn secure(self) -> Self {
        self.referrer_policy(attributes::ReferrerPolicy::NoReferrer)
            .rel(vec![attributes::Rel::NoOpener, attributes::Rel::NoReferrer])
    }
}

attr_helpers!(area =>
    alt(String, "alt"), coords(String, "coords"), download(bool, "download"),
    href(String, "href"), ping(String, "ping"), referrer_policy(attributes::ReferrerPolicy, "referrerpolicy"),
    rel(attributes::Rel, "rel"), shape(attributes::Shape, "shape"), target(attributes::Target, "target")
);
attr_helpers!(audio =>
    auto_play(bool, "autoplay"), controls(bool, "controls"), controls_list(attributes::ControlsList, "controlslist"),
    cross_origin(attributes::CrossOrigin, "crossorigin"), disable_remote_playback(bool, "disableremoteplayback"),
    loop_audio(bool, "loop"), muted(bool, "muted"), preload(attributes::ContentPreload, "preload"), src(String, "src")
);
attr_helpers!(blockquote => cite(String, "cite"));
attr_helpers!(button =>
    command(attributes::Command, "command"), command_for(Id, "commandfor"),
    disabled(bool, "disabled"), form(Id, "form"), form_action(String, "formaction"),
    form_encoding_type(attributes::EncodingType, "formenvtype"), form_method(attributes::FormMethod, "formmethod"),
    form_no_validate(bool, "formnovalidate"), form_target(attributes::Target, "formtarget"),
    name(String, "name"), popover_target(Id, "popovertarget"),
    popover_target_action(attributes::PopoverAction, "popovertargetaction"), button_type(attributes::ButtonType, "type"), value(String, "value")
);
attr_helpers!(canvas => height(attributes::Integer, "height"), width(attributes::Integer, "width"));
attr_helpers!(col => span(attributes::Integer, "span"));
attr_helpers!(colgroup => span(attributes::Integer, "span"));
attr_helpers!(data => value(String, "data"));
attr_helpers!(del => cite(String, "cite"));
attr_helpers!(details => open(bool, "open"), name(String, "name"));
attr_helpers!(dialog => open(bool, "open"));
attr_helpers!(embed =>
    height(attributes::Integer, "height"), width(attributes::Integer, "width"),
    src(String, "src"), mime_type(String, "type")
);
attr_helpers!(fieldset => disabled(bool, "disabled"), form(Id, "form"), name(String, "name"));
attr_helpers!(form =>
    auto_complete(attributes::OnOff, "autocomplete"), name(String, "name"), rel(attributes::Rel, "rel"),
    action(String, "action"), encoding_type(attributes::EncodingType, "enctype", "enctype"), method(attributes::FormMethod, "method"),
    no_validate(bool, "novalidate"), target(attributes::Target, "target")
);

attr_helpers!(iframe =>
    height(attributes::Integer, "height"), loading(attributes::Loading, "loading"),
    name(String, "name"), referrer_policy(attributes::ReferrerPolicy, "referrerpolicy"),
    sandbox(attributes::SandboxAllow, "sandbox"), src(String, "src"), srcdoc(String, "srcdoc"), width(attributes::Integer, "width")
);

impl<C: State> HtmlElement<C, TagIframe> {
    /// add `referrerpolicy="no-referrer" sandbox="" credentialless`
    #[inline]
    pub fn secure(self) -> Self {
        self.referrer_policy(attributes::ReferrerPolicy::NoReferrer)
            .sandbox(Vec::<attributes::SandboxAllow>::new())
            // This is a experimental attribute, so no helper
            .attr("credentialless", true)
    }
}

attr_helpers!(img =>
    alt(String, "alt"), cross_origin(attributes::CrossOrigin, "crossorigin"), decoding(attributes::ImageDecoding, "decoding"),
    fetch_priority(attributes::FetchPriority, "fetchpriority"), height(attributes::Integer, "height"), is_map(bool, "ismap"),
    loading(attributes::Loading, "loading"), referrer_policy(attributes::ReferrerPolicy, "referrerpolicy"),
    src(String, "src"), width(attributes::Integer, "width"), use_map(String, "usemap")
);

attr_helpers!(ins => cite(String, "cite"));
attr_helpers!(label => is_for(Id, "for"));
attr_helpers!(li => value(attributes::Integer, "value"));
attr_helpers!(map => name(String, "name"));

attr_helpers!(meter =>
    value(attributes::Float, "value"),
    min(attributes::Float, "min"), max(attributes::Float, "max"),
    high(attributes::Float, "high"), low(attributes::Float, "low"),
    optimum(attributes::Float, "optimum"),
    form(Id, "form")
);

attr_helpers!(object =>
    data(String, "data"), form(Id, "form"), height(attributes::Integer, "height"),
    name(String, "name"), object_type(String, "type"), width(attributes::Integer, "width")
);

attr_helpers!(ol =>
    reversed(bool, "reversed"), start(attributes::Integer, "start"), numeric_type(attributes::ListNumberingKind, "type")
);

attr_helpers!(optgroup => disabled(bool, "disabled"), label(String, "label"));
attr_helpers!(option =>
    disabled(bool, "disabled"), label(String, "label"),
    selected(bool, "selected"), value(String, "value")
);
attr_helpers!(output => is_for(Vec<Id>, "for"), form(Id, "form"), name(String, "name"));
attr_helpers!(progress => max(attributes::Float, "max"), values(attributes::Float, "value"));
attr_helpers!(q => cite(String, "cite"));
attr_helpers!(select =>
    auto_complete(attributes::AutoComplete, "autocomplete"),
    disabled(bool, "disabled"), form(Id, "form"), multiple(bool, "multiple"),
    name(String, "name"), required(bool, "required"), size(attributes::Integer, "size")
);

attr_helpers!(source =>
    source_type(String, "type"), src(String, "src"),
    height(attributes::Integer, "height"), width(attributes::Integer, "width")
);
attr_helpers!(textarea =>
    auto_complete(attributes::AutoComplete, "autocomplete"),
    auto_correct(attributes::OnOff, "autocorrect"), columns(attributes::Integer, "cols"),
    direction_name(String, "dirname"), disabled(bool, "disabled"), form(Id, "form"), max_length(attributes::Integer, "maxlength"),
    min_length(attributes::Integer, "min_length"), name(String, "name"), placeholder(String, "placeholder"),
    read_only(bool, "readonly"), required(bool, "required"), rows(attributes::Integer, "rows"),
    wrap(attributes::Wrap, "wrap")
);

attr_helpers!(th =>
    abbreviated(String, "abbr"), column_span(attributes::Integer, "colspan"), headers(Vec<Id>, "headers"),
    row_span(attributes::Integer, "row_span"), scope(attributes::TableHeadingScope, "scope")
);

// todo: <time>
attr_helpers!(track =>
    default(bool, "default"), kind(attributes::TrackKind, "kind"),
    label(String, "label"), src(String, "src"), src_language(String, "srclang")
);

attr_helpers!(video =>
    auto_play(bool, "autoplay"), controls(bool, "controls"), controls_list(attributes::ControlsList, "controlslist"),
    cross_origin(attributes::CrossOrigin, "crossorigin"), disable_picture_in_picture(bool, "disablepictureinpicture"), disable_remote_playback(bool, "disableremoteplayback"),
    height(attributes::Integer, "height"), loop_video(bool, "loop"), muted(bool, "muted"),
    plays_inline(bool, "playsinline"), poster(String, "poster"),
    preload(attributes::ContentPreload, "preload"), src(String, "src"), width(attributes::Integer, "width")
);
