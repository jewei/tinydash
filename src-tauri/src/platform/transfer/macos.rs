use crate::launcher::transfer::{DragCompletion, DragOutcome};
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};
use std::{cell::RefCell, path::PathBuf};

thread_local! {
    static SOURCE: RefCell<Option<Retained<ResultDragSource>>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Option<DragCompletion>>]
    struct ResultDragSource;

    unsafe impl NSObjectProtocol for ResultDragSource {}
    unsafe impl NSDraggingSource for ResultDragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operations(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            NSDragOperation::Copy
        }
        #[unsafe(method(ignoreModifierKeysForDraggingSession:))]
        fn ignore_modifiers(&self, _session: &NSDraggingSession) -> bool { true }
        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn ended(&self, _session: &NSDraggingSession, _point: NSPoint, operation: NSDragOperation) {
            let _keep_alive = self.retain();
            let completion = self.ivars().borrow_mut().take();
            SOURCE.with(|source| { source.borrow_mut().take(); });
            if let Some(completion) = completion {
                completion(Ok(if operation.is_empty() { DragOutcome::Cancelled } else { DragOutcome::Dropped }));
            }
        }
    }
);

pub fn drag(
    window: &tauri::WebviewWindow,
    path: PathBuf,
    complete: DragCompletion,
) -> Result<(), String> {
    let main = MainThreadMarker::new().ok_or("Native dragging requires the main thread.")?;
    if NSEvent::pressedMouseButtons() & 1 == 0 {
        complete(Ok(DragOutcome::Cancelled));
        return Ok(());
    }
    let raw = window.ns_view().map_err(|e| e.to_string())?;
    if raw.is_null() {
        return Err("The launcher view is unavailable.".into());
    }
    // Tauri owns the view. This function runs on its main thread.
    let view = unsafe { &*raw.cast::<NSView>() };
    let native_window = view.window().ok_or("The launcher window is unavailable.")?;
    let point = native_window.mouseLocationOutsideOfEventStream();
    let event = {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            NSEventType::LeftMouseDragged, point, NSEventModifierFlags::empty(), 0.0,
            native_window.windowNumber(), None, 0, 1, 1.0,
        )
    }.ok_or("Could not create a native drag event.")?;
    let path = NSString::from_str(path.to_str().ok_or("The path is not valid UTF-8.")?);
    let url = NSURL::fileURLWithPath(&path);
    let item = NSDraggingItem::initWithPasteboardWriter(
        NSDraggingItem::alloc(),
        ProtocolObject::from_ref(&*url),
    );
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&path);
    let location = view.convertPoint_fromView(point, None);
    unsafe {
        item.setDraggingFrame_contents(
            NSRect::new(
                NSPoint::new(location.x - 16.0, location.y - 16.0),
                NSSize::new(32.0, 32.0),
            ),
            Some(&*icon),
        );
    }
    let allocated = ResultDragSource::alloc(main).set_ivars(RefCell::new(Some(complete)));
    let source: Retained<ResultDragSource> = unsafe { msg_send![super(allocated), init] };
    SOURCE.with(|current| *current.borrow_mut() = Some(source.clone()));
    let items = NSArray::from_retained_slice(&[item]);
    view.beginDraggingSessionWithItems_event_source(
        &items,
        &event,
        ProtocolObject::from_ref(&*source),
    );
    Ok(())
}
