use crate::{Widget, WidgetPod, LifeCycle, EventCtx, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Selector, WidgetId, WidgetExt, Rect, Point, WindowDesc, WindowHandle, Data, Lens, WindowId};
use std::rc::Rc;
use std::ops::Deref;
use std::borrow::BorrowMut;
use std::any::Any;
use crate::win_handler::AppState;
use druid_shell::Error;
use std::marker::PhantomData;
use std::cell::RefCell;
use crate::app::{WindowConfig, PendingWindow};

// We have to have no generics, as both ends would need to know them.
// So we erase everything to ()
pub struct SubWindowRequirement {
    host_id: WidgetId,
    port_id: WidgetId,
    pub(crate) sub_window_host: Box<dyn Widget<()>>,
    pub(crate) window_config: WindowConfig,
}

pub struct SubWindowRequirementTransfer{
    pub(crate) inner: RefCell<Option<SubWindowRequirement>>
}

impl SubWindowRequirementTransfer {
    pub fn new(inner: SubWindowRequirement) -> Self {
        Self { inner: RefCell::new(Some(inner)) }
    }
}

struct UnitLens<T>{
    phantom_t: PhantomData<T>
}

impl<T> UnitLens<T> {
    pub fn new() -> Self {
        UnitLens { phantom_t: Default::default() }
    }
}

impl <T> Lens<T, ()> for UnitLens<T>{
    fn with<V, F: FnOnce(&()) -> V>(&self, _data: &T, f: F) -> V {
        f(&())
    }
    fn with_mut<V, F: FnOnce(&mut ()) -> V>(&self, _data: &mut T, f: F) -> V {
        f(&mut ())
    }
}

impl SubWindowRequirement {
    pub fn make_requirement_and_port<U: Data, W: Widget<U> + 'static >(window_config: WindowConfig, widget: W, data: U) -> (Self, SubWindowPort<U>) {
        let host_id = WidgetId::next();
        let port_id = WidgetId::next();

        let sub_window_host = SubWindowHost::new(host_id, port_id, data, widget).boxed();
        let requirement = SubWindowRequirement { host_id: WidgetId::next(), port_id: WidgetId::next(), sub_window_host,  window_config };
        let port = SubWindowPort::new(port_id, host_id);
        (requirement, port)
    }

    pub (crate) fn make_sub_window<T: Data>(mut self, app_state: &mut AppState<T>) -> Result<WindowHandle, Error> {
        let pending = PendingWindow::new_from_boxed(self.sub_window_host.lens( UnitLens::new() ).boxed() );
        app_state.build_native_window(WindowId::next(), pending, self.window_config)
    }

}

pub struct SubWindowPort<U>{
    id: WidgetId,
    host_id: WidgetId,
    phantom_u: PhantomData<U>
}

impl<U> SubWindowPort<U> {
    pub fn new(id: WidgetId, host_id: WidgetId) -> Self {
        SubWindowPort { id, host_id, phantom_u: Default::default() }
    }
}

pub struct SubWindowHost<U, W: Widget<U>>{
    id: WidgetId,
    port_id: WidgetId,
    data: U,
    child: WidgetPod<U, W>,
}

impl<U, W: Widget<U>> SubWindowHost<U, W> {
    pub fn new(id: WidgetId, port_id: WidgetId, data: U, widget: W) -> Self {
        SubWindowHost {id, port_id, data, child: WidgetPod::new(widget)}
    }
}

pub(crate) const SUB_WINDOW_PORT_TO_HOST: Selector<Box<dyn Any>> =
    Selector::new("druid-builtin.port_to_host");

pub(crate) const SUB_WINDOW_HOST_TO_PORT: Selector<Box<dyn Any>> =
    Selector::new("druid-builtin.host_to_port");

// Should this even be a widget. Just needs to be delegated to
impl <U: Data> Widget<U> for SubWindowPort<U> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut U, env: &Env) {
        match event {
            Event::Command(cmd) if cmd.is(SUB_WINDOW_HOST_TO_PORT) =>{
                if let Some(update) = cmd.get_unchecked(SUB_WINDOW_HOST_TO_PORT).downcast_ref::<U>(){
                     *data = (*update).clone();
                }
                ctx.set_handled();
             },
            _=>{}
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &U, env: &Env) {

    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &U, data: &U, env: &Env) {
        if !old_data.same(data){
            ctx.submit_command(SUB_WINDOW_PORT_TO_HOST.with( Box::new(data.clone()) ), self.host_id)
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &U, env: &Env) -> Size {

        Size::ZERO
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &U, env: &Env) {

    }

    fn id(&self) -> Option<WidgetId> {
        Some(self.id)
    }
}


impl <U: Data, W: Widget<U>>  Widget<()> for SubWindowHost<U, W>{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut (), env: &Env) {
        match event{
            Event::Command(cmd) if cmd.is(SUB_WINDOW_PORT_TO_HOST) =>{
                if let Some(update) = cmd.get_unchecked(SUB_WINDOW_PORT_TO_HOST).downcast_ref::<U>(){
                    self.data = update.clone();
                    let mut update_ctx = UpdateCtx{
                         state: ctx.state,
                         widget_state: ctx.widget_state
                    };
                    self.child.update(&mut update_ctx, &self.data, env); // Should env be copied around too?
                }
                ctx.set_handled();
            },
            _=>{
                let old = self.data.clone(); // Could avoid this by keeping two or if we could ask widget pod
                self.child.event(ctx, event, &mut self.data, env);
                // This update is happening before process commands. Not sure if that matters.
                let mut update_ctx = UpdateCtx{
                    state: ctx.state,
                    widget_state: ctx.widget_state
                };
                self.child.update(&mut update_ctx, &self.data, env);
                if !old.same(&self.data){
                    ctx.submit_command(SUB_WINDOW_HOST_TO_PORT.with( Box::new(self.data.clone()) ), self.port_id)
                }
            }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &(), env: &Env) {
        self.child.lifecycle(ctx, event, &self.data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &(), data: &(), env: &Env) {
        // Can't use this change
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &(), env: &Env) -> Size {
        let size = self.child.layout(ctx, bc, &self.data, env);
        self.child.set_layout_rect(ctx, &self.data, env, Rect::from_origin_size(Point::ORIGIN, size));
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &(), env: &Env) {
        self.child.paint_raw(ctx, &self.data, env);
    }

    fn id(&self) -> Option<WidgetId> {
        Some(self.id)
    }
}