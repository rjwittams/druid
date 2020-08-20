use crate::app::{PendingWindow, WindowConfig};
use crate::lens::UnitLens;
use crate::win_handler::AppState;
use crate::{Data, Widget, WidgetExt, WidgetId, WindowHandle, WindowId};
use druid_shell::Error;

// We have to have no generics, as both ends would need to know them.
// So we erase everything to ()
pub struct SubWindowRequirement {
    pub(crate) host_id: Option<WidgetId>, // Present if updates should be sent from the pod to the sub window.
    pub(crate) sub_window_root: Box<dyn Widget<()>>,
    pub(crate) window_config: WindowConfig,
    pub window_id: WindowId,
}

impl SubWindowRequirement {
    pub fn new(
        host_id: Option<WidgetId>,
        sub_window_root: Box<dyn Widget<()>>,
        window_config: WindowConfig,
        window_id: WindowId,
    ) -> Self {
        SubWindowRequirement {
            host_id,
            sub_window_root,
            window_config,
            window_id,
        }
    }

    pub(crate) fn make_sub_window<T: Data>(
        self,
        app_state: &mut AppState<T>,
    ) -> Result<WindowHandle, Error> {
        let pending =
            PendingWindow::new_from_boxed(self.sub_window_root.lens(UnitLens::default()).boxed());
        app_state.build_native_window(self.window_id, pending, self.window_config)
    }
}
