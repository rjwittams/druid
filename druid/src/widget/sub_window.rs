use crate::{Widget, WidgetPod, LifeCycle, EventCtx, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Selector, WidgetId, WidgetExt, Rect, Point, WindowDesc, WindowHandle, Data, Lens};
use std::rc::Rc;
use std::ops::Deref;
use std::borrow::BorrowMut;
use std::any::Any;
use crate::win_handler::AppState;
use druid_shell::Error;
use std::marker::PhantomData;
use std::cell::RefCell;

// We have to have no generics, as both ends would need to know them.
// So we erase everything to ()
pub struct SubWindowRequirement {
    host_id: WidgetId,
    port_id: WidgetId,
    pub(crate) window_desc: WindowDesc<()>,
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
    fn with<V, F: FnOnce(&()) -> V>(&self, data: &T, f: F) -> V {
        f(&())
    }
    fn with_mut<V, F: FnOnce(&mut ()) -> V>(&self, data: &mut T, f: F) -> V {
        f(&mut ())
    }
}

impl SubWindowRequirement {
    pub fn make_requirement_and_port<U: Data>(data: U, window_desc: WindowDesc<U>) -> (Self, SubWindowPort<U>) {
        let host_id = WidgetId::next();
        let port_id = WidgetId::next();

        // Annoying that we have 2 levels of boxing here
        let unit_window: WindowDesc<()> = window_desc.map_widget(|widget|{
            let pod = WidgetPod::new(widget);
            SubWindowHost::new(host_id, port_id, data, pod).boxed()
        });

        let requirement = SubWindowRequirement { host_id: WidgetId::next(), port_id: WidgetId::next(), window_desc: unit_window };
        let port = SubWindowPort::new(port_id, host_id);
        (requirement, port)
    }

    pub (crate) fn make_sub_window<T: Data>(mut self, app_state: &mut AppState<T>) -> Result<WindowHandle, Error> {
        let app_level_window_desc =  self.window_desc.map_widget(|widget|{
            widget.lens( UnitLens::new() ).boxed()
        });
        app_level_window_desc.build_native(app_state)
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

pub struct SubWindowHost<U>{
    id: WidgetId,
    port_id: WidgetId,
    data: U,
    child: WidgetPod<U, Box<dyn Widget<U>>>,
}

impl<U> SubWindowHost<U> {
    pub fn new(id: WidgetId, port_id: WidgetId, data: U, widget: WidgetPod<U, Box<dyn Widget<U>>>) -> Self {
        SubWindowHost {id, port_id, data, child: widget}
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
                    log::info!("Got update from host");
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
            log::info!("Sending update from port to id {:?}", self.host_id);
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


impl <U:Data> Widget<()> for SubWindowHost<U>{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut (), env: &Env) {
        match event{
            Event::Command(cmd) if cmd.is(SUB_WINDOW_PORT_TO_HOST) =>{
                log::info!("Got update from port my id {:?}", self.id );
                if let Some(update) = cmd.get_unchecked(SUB_WINDOW_PORT_TO_HOST).downcast_ref::<U>(){
                    self.data = update.clone();
                    log::info!("Content update from port my id {:?}", self.id );
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
                    log::info!("Sending update from host");
                    ctx.submit_command(SUB_WINDOW_HOST_TO_PORT.with( Box::new(self.data.clone()) ), self.port_id)
                }
            }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &(), env: &Env) {
        self.child.lifecycle(ctx, event, &self.data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &(), data: &(), env: &Env) {
        // Can't use this change afaict
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