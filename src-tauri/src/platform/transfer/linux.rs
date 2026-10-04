use crate::launcher::transfer::{DragCompletion, DragOutcome};

pub fn share(
    _window: &tauri::WebviewWindow,
    _item: crate::launcher::transfer::ShareItem,
    _complete: crate::launcher::transfer::ShareCompletion,
) -> Result<(), String> {
    Err("Native Share is unavailable on this desktop. Use the copy actions instead.".into())
}
use gtk::{gdk, glib::SignalHandlerId, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

pub fn drag(
    window: &tauri::WebviewWindow,
    path: PathBuf,
    complete: DragCompletion,
) -> Result<(), String> {
    if crate::platform::is_wayland() {
        return Err(
            "Native result dragging requires X11. Use Copy file or Show in folder on Wayland."
                .into(),
        );
    }
    let window = window.gtk_window().map_err(|e| e.to_string())?;
    let native = window
        .window()
        .ok_or("The launcher window is unavailable.")?;
    let pointer = native
        .display()
        .default_seat()
        .and_then(|seat| seat.pointer())
        .ok_or("No pointer device is available.")?;
    if !native
        .device_position(&pointer)
        .3
        .contains(gdk::ModifierType::BUTTON1_MASK)
    {
        complete(Ok(DragOutcome::Cancelled));
        return Ok(());
    }
    let uri = url::Url::from_file_path(&path)
        .map_err(|_| "Invalid file path")?
        .to_string();
    let complete = Rc::new(RefCell::new(Some(complete)));
    let handlers: Rc<RefCell<Vec<SignalHandlerId>>> = Rc::default();
    let failed = Rc::new(Cell::new(false));
    let targets = gtk::TargetList::new(&[gtk::TargetEntry::new(
        "text/uri-list",
        gtk::TargetFlags::empty(),
        0,
    )]);
    handlers
        .borrow_mut()
        .push(window.connect_drag_data_get(move |_, _, selection, _, _| {
            selection.set_uris(&[&uri]);
        }));
    let did_fail = failed.clone();
    handlers
        .borrow_mut()
        .push(window.connect_drag_failed(move |_, _, _| {
            did_fail.set(true);
            gtk::glib::Propagation::Proceed
        }));
    let end_handlers = handlers.clone();
    let end_complete = complete.clone();
    handlers
        .borrow_mut()
        .push(window.connect_drag_end(move |widget, context| {
            let callback = end_complete.borrow_mut().take();
            for handler in end_handlers.borrow_mut().drain(..) {
                widget.disconnect(handler);
            }
            if let Some(callback) = callback {
                callback(Ok(
                    if failed.get() || context.selected_action().is_empty() {
                        DragOutcome::Cancelled
                    } else {
                        DragOutcome::Dropped
                    },
                ));
            }
        }));
    if window
        .drag_begin_with_coordinates(&targets, gdk::DragAction::COPY, 1, None, -1, -1)
        .is_none()
    {
        for handler in handlers.borrow_mut().drain(..) {
            window.disconnect(handler);
        }
        return Err("Could not start dragging this item.".into());
    }
    Ok(())
}
