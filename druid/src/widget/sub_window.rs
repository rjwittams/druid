use crate::app::{PendingWindow, WindowConfig};
use crate::command::sys::SUB_WINDOW_PARENT_TO_HOST;
use crate::commands::SUB_WINDOW_HOST_TO_PARENT;
use crate::win_handler::AppState;
use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx,
    Point, Rect, Size, UpdateCtx, Widget, WidgetExt, WidgetId, WidgetPod, WindowHandle, WindowId,
};
use druid_shell::Error;
use std::marker::PhantomData;
use std::ops::Deref;

// We have to have no generics, as both ends would need to know them.
// So we erase everything to ()
pub struct SubWindowRequirement {
    pub(crate) host_id: Option<WidgetId>, // Present if updates should be sent from the pod to the sub window.
    pub(crate) sub_window_host: Box<dyn Widget<()>>,
    pub(crate) window_config: WindowConfig,
    pub window_id: WindowId,
}

struct UnitLens<T> {
    phantom_t: PhantomData<T>,
}

impl<T> UnitLens<T> {
    pub fn new() -> Self {
        UnitLens {
            phantom_t: Default::default(),
        }
    }
}

impl<T> Lens<T, ()> for UnitLens<T> {
    fn with<V, F: FnOnce(&()) -> V>(&self, _data: &T, f: F) -> V {
        f(&())
    }
    fn with_mut<V, F: FnOnce(&mut ()) -> V>(&self, _data: &mut T, f: F) -> V {
        f(&mut ())
    }
}

impl SubWindowRequirement {
    pub fn new<U: Data, W: Widget<U> + 'static>(
        parent_id: WidgetId,
        window_config: WindowConfig,
        sync: bool,
        widget: W,
        data: U,
    ) -> Self {
        let host_id = WidgetId::next();
        let sub_window_host = SubWindowHost::new(host_id, parent_id, sync, data, widget).boxed();
        SubWindowRequirement {
            host_id: if sync { Some(host_id) } else { None },
            sub_window_host,
            window_config,
            window_id: WindowId::next(),
        }
    }

    pub(crate) fn make_sub_window<T: Data>(
        self,
        app_state: &mut AppState<T>,
    ) -> Result<WindowHandle, Error> {
        let pending =
            PendingWindow::new_from_boxed(self.sub_window_host.lens(UnitLens::new()).boxed());
        app_state.build_native_window(self.window_id, pending, self.window_config)
    }
}

pub struct SubWindowHost<U, W: Widget<U>> {
    id: WidgetId,
    parent_id: WidgetId,
    sync: bool,
    data: U,
    child: WidgetPod<U, W>,
}

impl<U, W: Widget<U>> SubWindowHost<U, W> {
    pub fn new(id: WidgetId, port_id: WidgetId, sync: bool, data: U, widget: W) -> Self {
        SubWindowHost {
            id,
            parent_id: port_id,
            sync,
            data,
            child: WidgetPod::new(widget),
        }
    }
}

impl<U: Data, W: Widget<U>> Widget<()> for SubWindowHost<U, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, _data: &mut (), env: &Env) {
        match event {
            Event::Command(cmd) if self.sync && cmd.is(SUB_WINDOW_PARENT_TO_HOST) => {
                if let Some(update) = cmd
                    .get_unchecked(SUB_WINDOW_PARENT_TO_HOST)
                    .downcast_ref::<U>()
                {
                    self.data = update.deref().clone();
                    let mut update_ctx = UpdateCtx {
                        state: ctx.state,
                        widget_state: ctx.widget_state,
                    };
                    self.child.update(&mut update_ctx, &self.data, env); // Should env be copied around too?
                }
                ctx.set_handled();
            }
            _ => {
                let old = self.data.clone(); // Could avoid this by keeping two or if we could ask widget pod
                self.child.event(ctx, event, &mut self.data, env);
                // This update is happening before process commands. Not sure if that matters.
                let mut update_ctx = UpdateCtx {
                    state: ctx.state,
                    widget_state: ctx.widget_state,
                };
                self.child.update(&mut update_ctx, &self.data, env);
                if self.sync && !old.same(&self.data) {
                    ctx.submit_command(
                        SUB_WINDOW_HOST_TO_PARENT.with(Box::new(self.data.clone())),
                        self.parent_id,
                    )
                }
            }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(), env: &Env) {
        self.child.lifecycle(ctx, event, &self.data, env)
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(), _data: &(), _env: &Env) {
        // Can't use this change
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &(), env: &Env) -> Size {
        let size = self.child.layout(ctx, bc, &self.data, env);
        self.child.set_layout_rect(
            ctx,
            &self.data,
            env,
            Rect::from_origin_size(Point::ORIGIN, size),
        );
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &(), env: &Env) {
        self.child.paint_raw(ctx, &self.data, env);
    }

    fn id(&self) -> Option<WidgetId> {
        Some(self.id)
    }
}
