use crate::app::WindowConfig;
use crate::command::sys::SUB_WINDOW_PARENT_TO_HOST;
use crate::commands::SUB_WINDOW_HOST_TO_PARENT;
use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    Point, Rect, Size, SubWindowRequirement, UpdateCtx, Widget, WidgetExt, WidgetId, WidgetPod,
    WindowId,
};
use std::ops::Deref;

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

    pub fn make_requirement(
        parent_id: WidgetId,
        window_config: WindowConfig,
        sync: bool,
        widget: W,
        data: U,
    ) -> SubWindowRequirement
    where
        W: 'static,
        U: Data,
    {
        let host_id = WidgetId::next();
        let sub_window_host = SubWindowHost::new(host_id, parent_id, sync, data, widget).boxed();
        SubWindowRequirement {
            host_id: if sync { Some(host_id) } else { None },
            sub_window_root: sub_window_host,
            window_config,
            window_id: WindowId::next(),
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
                } else {
                    log::warn!("Received a sub window parent to host command that could not be unwrapped. \
                    This could mean that the sub window you requested and the enclosing widget pod that you opened it from do not share a common data type. \
                    Make sure you have a widget pod between your requesting widget and any lenses." )
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
