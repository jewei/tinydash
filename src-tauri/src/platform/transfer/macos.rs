use crate::launcher::transfer::{DragCompletion, DragOutcome};
use crate::launcher::transfer::{ShareCompletion, ShareItem};
use objc2::{
    AnyThread, DefinedClass, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject,
};
use objc2_app_kit::{
    NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWorkspace,
};
use objc2_app_kit::{
    NSSharingService, NSSharingServiceDelegate, NSSharingServicePicker,
    NSSharingServicePickerDelegate,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};
use objc2_foundation::{NSError, NSRectEdge};
use std::{cell::RefCell, path::PathBuf};

thread_local! {
    static SOURCE: RefCell<Option<Retained<ResultDragSource>>> = const { RefCell::new(None) };
    // The picker uses a weak delegate. Keep both alive until cancellation or completion.
    static SHARE: RefCell<Option<(Retained<NSSharingServicePicker>, Retained<ResultShareDelegate>)>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RefCell<Option<ShareCompletion>>]
    struct ResultShareDelegate;

    unsafe impl NSObjectProtocol for ResultShareDelegate {}
    unsafe impl NSSharingServicePickerDelegate for ResultShareDelegate {
        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn chose(&self, _picker: &NSSharingServicePicker, service: Option<&NSSharingService>) {
            if service.is_none() { self.finish(Ok(())); }
        }
        #[unsafe(method_id(sharingServicePicker:delegateForSharingService:))]
        fn service_delegate(&self, _picker: &NSSharingServicePicker, _service: &NSSharingService) -> Option<Retained<ProtocolObject<dyn NSSharingServiceDelegate>>> {
            Some(ProtocolObject::from_retained(self.retain()))
        }
    }
    unsafe impl NSSharingServiceDelegate for ResultShareDelegate {
        #[unsafe(method(sharingService:didShareItems:))]
        fn completed(&self, _service: &NSSharingService, _items: &NSArray) { self.finish(Ok(())); }
        #[unsafe(method(sharingService:didFailToShareItems:error:))]
        fn failed(&self, _service: &NSSharingService, _items: &NSArray, error: &NSError) {
            self.finish(Err(error.localizedDescription().to_string()));
        }
    }
);

impl ResultShareDelegate {
    fn finish(&self, result: Result<(), String>) {
        let _keep_alive = self.retain();
        let completion = self.ivars().borrow_mut().take();
        SHARE.with(|session| {
            session.borrow_mut().take();
        });
        if let Some(completion) = completion {
            completion(result);
        }
    }
}

pub fn share(
    window: &tauri::WebviewWindow,
    item: ShareItem,
    complete: ShareCompletion,
) -> Result<(), String> {
    let main = MainThreadMarker::new().ok_or("Native sharing requires the main thread.")?;
    let object: Retained<objc2::runtime::AnyObject> = match item {
        ShareItem::File(path) => NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().ok_or("Invalid file path")?,
        ))
        .into_super(),
        ShareItem::Url(url) => NSURL::URLWithString(&NSString::from_str(&url))
            .ok_or("Invalid share URL")?
            .into_super(),
        ShareItem::Text(text) => NSString::from_str(&text).into_super(),
    }
    .into_super();
    let raw = window.ns_view().map_err(|e| e.to_string())?;
    if raw.is_null() {
        return Err("The launcher view is unavailable.".into());
    }
    let view = unsafe { &*raw.cast::<NSView>() };
    let items = NSArray::from_retained_slice(&[object]);
    // All objects above are NSString/NSURL and support NSPasteboardWriting.
    let picker =
        unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items) };
    let allocated = ResultShareDelegate::alloc(main).set_ivars(RefCell::new(Some(complete)));
    let delegate: Retained<ResultShareDelegate> = unsafe { msg_send![super(allocated), init] };
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    SHARE.with(|session| *session.borrow_mut() = Some((picker.clone(), delegate)));
    let bounds = view.bounds();
    picker.showRelativeToRect_ofView_preferredEdge(
        NSRect::new(
            NSPoint::new(bounds.size.width - 80.0, 40.0),
            NSSize::new(1.0, 1.0),
        ),
        view,
        NSRectEdge::MinY,
    );
    Ok(())
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
