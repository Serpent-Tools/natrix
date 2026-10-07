//! Implementation of event traits as well as a helper trait for event handlers.

use wasm_bindgen::JsCast;

use crate::reactivity::context::EventCtx;

/// Trait for converting a struct to needed event info.
pub trait Event {
    /// The js event the handler gets
    type JsEvent: JsCast;
    /// The actual name
    const EVENT_NAME: &str;
}
/// Utility trait for defining event handler in arguments.
///
/// When defining a render function it is much easier to use `impl EventHandler<C, E>` than writing out
/// the whole function trait yourself.
///
/// ```
/// # use natrix::prelude::*;
/// # use natrix::dom::EventHandler;
/// fn my_button<C: State>(click: impl EventHandler<C, events::Click>) -> impl Element<C> {
///     e::button().on::<events::Click>(click)
/// }
/// ```
pub trait EventHandler<C, E: Event> {
    /// Return self, but constrained to the expected types
    fn func(self) -> impl Fn(EventCtx<C>, E::JsEvent) + 'static;
}
impl<C, E: Event, F: Fn(EventCtx<C>, E::JsEvent) + 'static> EventHandler<C, E> for F {
    fn func(self) -> impl Fn(EventCtx<C>, E::JsEvent) + 'static {
        self
    }
}

/// Implement and define a `Event`
macro_rules! impl_event {
    ($ty:ident => $name:literal, $handler:ident) => {
        #[doc = $name]
        pub struct $ty;

        impl Event for $ty {
            type JsEvent = web_sys::$handler;
            const EVENT_NAME: &str = $name;
        }
    };
}

impl_event!(AnimationCancel => "animationcancel", AnimationEvent);
impl_event!(AnimationEnd => "animationend", AnimationEvent);
impl_event!(AnimationIteration => "animationiteration", AnimationEvent);
impl_event!(AnimationStart => "animationstart", AnimationEvent);
impl_event!(AuxClick => "auxclick", PointerEvent);
impl_event!(BeforeInput => "beforeinput", InputEvent);
impl_event!(Blur => "blur", FocusEvent);
impl_event!(Click => "click", PointerEvent);
impl_event!(CompositionEnd => "compositionend", CompositionEvent);
impl_event!(CompositionStart => "compositionstart", CompositionEvent);
impl_event!(CompositionUpdate => "compositionupdate", CompositionEvent);
impl_event!(ContentVisibilityAutoStateChange => "contentvisibilityautostatechange", Event);
impl_event!(ContextMenu => "contextmenu", PointerEvent);
impl_event!(Copy => "copy", ClipboardEvent);
impl_event!(Cut => "cut", ClipboardEvent);
impl_event!(DoubleClick => "dblclick", MouseEvent);
impl_event!(Focus => "focus", FocusEvent);
impl_event!(FocusIn => "focusin", FocusEvent);
impl_event!(FocusOut => "focusout", FocusEvent);
impl_event!(FullscreenChange => "fullscreenchange", Event);
impl_event!(FullscreenError => "fullscreenerror", Event);
impl_event!(GotPointerCapture => "gotpointercapture", PointerEvent);
impl_event!(Input => "input", InputEvent);
impl_event!(KeyDown => "keydown", KeyboardEvent);
impl_event!(KeyUp => "keyup", KeyboardEvent);
impl_event!(LostPointerCapture => "lostpointercapture", PointerEvent);
impl_event!(MouseDown => "mousedown", MouseEvent);
impl_event!(MouseEnter => "mouseenter", MouseEvent);
impl_event!(MouseLeave => "mouseleave", MouseEvent);
impl_event!(MouseMove => "mousemove", MouseEvent);
impl_event!(MouseOut => "mouseout", MouseEvent);
impl_event!(MouseOver => "mouseover", MouseEvent);
impl_event!(MouseUp => "mouseup", MouseEvent);
impl_event!(Paste => "paste", ClipboardEvent);
impl_event!(PointerCancel => "pointercancel", PointerEvent);
impl_event!(PointerDown => "pointerdown", PointerEvent);
impl_event!(PointerEnter => "pointerenter", PointerEvent);
impl_event!(PointerLeave => "pointerleave", PointerEvent);
impl_event!(PointerMove => "pointermove", PointerEvent);
impl_event!(PointerOut => "pointerout", PointerEvent);
impl_event!(PointerOver => "pointerover", PointerEvent);
impl_event!(PointerUp => "pointerup", PointerEvent);
impl_event!(Scroll => "scroll", Event);
impl_event!(ScrollEnd => "scrollend", Event);
impl_event!(SecurityPolicyViolation => "securitypolicyviolation", Event);
impl_event!(TouchCancel => "touchcancel", TouchEvent);
impl_event!(TouchEnd => "touchend", TouchEvent);
impl_event!(TouchMove => "touchmove", TouchEvent);
impl_event!(TouchStart => "touchstart", TouchEvent);
impl_event!(TransitionCancel => "transitioncancel", TransitionEvent);
impl_event!(TransitionEnd => "transitionend", TransitionEvent);
impl_event!(TransitionRun => "transitionrun", TransitionEvent);
impl_event!(TransitionStart => "transitionstart", TransitionEvent);
impl_event!(Wheel => "wheel", WheelEvent);
