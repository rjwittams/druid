use crate::{BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle,
            LifeCycleCtx, PaintCtx, Selector, Size, UpdateCtx, Widget, Lens};
use std::marker::PhantomData;

pub trait WidgetWrapper<Wrapped, T> {
    fn wrapped(&self) -> &Wrapped;
    fn wrapped_mut(&mut self) -> &mut Wrapped;
}


impl <T, W: Widget<T>> WidgetWrapper<W, T> for W{
    fn wrapped(&self) -> &W {
        self
    }

    fn wrapped_mut(&mut self) -> &mut W {
        self
    }
}


// This is a two way binding between some data, and something it is controlling.
// Usually this will be synchronising one bit of information in each,
// eg one field of T bound to one 'property' of a controlled Widget.
pub trait Binding<T, Controlled> {
    type Change;
    fn apply_data_to_controlled(&self, data: &T, controlled: &mut Controlled, ctx: &mut UpdateCtx);
    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
    );
    fn apply_change_to_data(&self, controlled: &Controlled, data: &mut T, change: Self::Change, ctx: &mut EventCtx);
}

// Allows a cons-list (or HList) of bindings to be built up.
impl <T, Controlled,
    Bind1: Binding<T, Controlled>,
    Bind2:Binding<T, Controlled> > Binding<T, Controlled> for (Bind1, Bind2){
    type Change = (Option<Bind1::Change>, Option<Bind2::Change>);

    fn apply_data_to_controlled(&self, data: &T, controlled: &mut Controlled, ctx: &mut UpdateCtx) {
        self.0.apply_data_to_controlled(data, controlled, ctx);
        self.1.apply_data_to_controlled(data, controlled, ctx);
    }

    fn append_change_required(&self, controlled: &Controlled, data: &T, change: &mut Option<Self::Change>) {

        let (change0, change1)  = change.get_or_insert_with (||(None, None));
        self.0.append_change_required(controlled, data, change0);
        self.1.append_change_required(controlled, data, change1);
        if let Some((None, None)) = change{
            *change = None;
        }
    }

    fn apply_change_to_data(&self, controlled: &Controlled, data: &mut T, change: Self::Change, ctx: &mut EventCtx) {
        let (change0, change1) = change;

        if let Some(change0) = change0{
            self.0.apply_change_to_data(controlled, data, change0, ctx);
        }

        if let Some(change1) = change1{
            self.1.apply_change_to_data(controlled, data, change1, ctx);
        }

    }
}

pub struct DataToWidgetOnlyBinding<B>(pub B);

impl <T, Controlled, B: Binding<T, Controlled>> Binding<T, Controlled> for DataToWidgetOnlyBinding<B> {
    type Change = ();

    fn apply_data_to_controlled(&self, data: &T, controlled: &mut Controlled, ctx: &mut UpdateCtx) {
        self.0.apply_data_to_controlled(data, controlled, ctx);
    }

    fn append_change_required(&self, controlled: &Controlled, data: &T, change: &mut Option<Self::Change>) {

    }

    fn apply_change_to_data(&self, controlled: &Controlled, data: &mut T, change: Self::Change, ctx: &mut EventCtx) {

    }
}

pub struct WidgetToDataOnlyBinding<B>(B);

impl <T, Controlled, B: Binding<T, Controlled>> Binding<T, Controlled> for WidgetToDataOnlyBinding<B> {
    type Change = B::Change;

    fn apply_data_to_controlled(&self, data: &T, controlled: &mut Controlled, ctx: &mut UpdateCtx) {

    }

    fn append_change_required(&self, controlled: &Controlled, data: &T, change: &mut Option<Self::Change>) {
        self.0.append_change_required(controlled, data, change);
    }

    fn apply_change_to_data(&self, controlled: &Controlled, data: &mut T, change: Self::Change, ctx: &mut EventCtx) {
        self.0.apply_change_to_data(controlled, data, change, ctx);
    }
}

pub struct LensBinding<T, Controlled, Prop, LT: Lens<T, Prop>, LC: Lens<Controlled, Prop>>{
    lens_from_data: LT,
    lens_from_controlled: LC,
    phantom_t: PhantomData<T>,
    phantom_c: PhantomData<Controlled>,
    phantom_p: PhantomData<Prop>
}

impl<T, Controlled, Prop, LT: Lens<T, Prop>, LC: Lens<Controlled, Prop>> LensBinding<T, Controlled, Prop, LT, LC> {
    pub fn new(lens_from_data: LT, lens_from_controlled: LC) -> Self {
        LensBinding { lens_from_data, lens_from_controlled, phantom_t: Default::default(), phantom_c: Default::default(), phantom_p: Default::default()}
    }
}

impl <T, Controlled, Prop: Data, LT: Lens<T, Prop>, LC: Lens<Controlled, Prop>> Binding<T, Controlled> for LensBinding<T, Controlled, Prop, LT, LC>{
    type Change = Prop;

    fn apply_data_to_controlled(&self, data: &T, controlled: &mut Controlled, ctx: &mut UpdateCtx) {
        self.lens_from_data.with(data,|field_val|{
            self.lens_from_controlled.with_mut(controlled, |c_prop|{
                *c_prop = field_val.clone();
            })
        });
        ctx.request_paint();
    }

    fn append_change_required(&self, controlled: &Controlled, data: &T, change: &mut Option<Self::Change>) {
        self.lens_from_data.with(data, |field_val|{
            self.lens_from_controlled.with(controlled, |c_prop|{
                *change = if c_prop.same(field_val) {None} else { Some(c_prop.clone()) };
            })
        })
    }

    fn apply_change_to_data(&self, controlled: &Controlled, data: &mut T, change: Self::Change, ctx: &mut EventCtx) {
        self.lens_from_data.with_mut(data , |field| *field = change)
    }
}

pub struct BindingHost<
    T,
    U,
    Contained: WidgetWrapper<Controlled, U> + Widget<T>,
    Controlled: Widget<U>,
    B: Binding<T, Controlled>,
> {
    contained: Contained,
    binding: B,
    pending_change: Option<B::Change>,
    phantom_u: PhantomData<U>,
}

impl<
        T,
        U,
        Contained: WidgetWrapper<Controlled, U> + Widget<T>,
        Controlled: Widget<U>,
        B: Binding<T, Controlled>,
    > BindingHost<T, U, Contained, Controlled, B>
{
    pub fn new(
        contained: Contained,
        binding: B
    ) -> Self {
        BindingHost {
            contained,
            binding,
            pending_change: None,
            phantom_u: Default::default(),
        }
    }
}

const APPLY_BINDINGS: Selector = Selector::new("druid-builtin.apply-bindings");

impl<
        T: Data,
        U,
        Contained: WidgetWrapper<Controlled, U> + Widget<T>,
        Controlled: Widget<U>,
        B: Binding<T, Controlled>,
    > Widget<T> for BindingHost<T, U, Contained, Controlled, B>
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        // Changes that occurred in other methods
        if let Some(change) = self.pending_change.take() {
            self.binding
                .apply_change_to_data(self.contained.wrapped(), data, change, ctx)
        }

        match event {
            Event::Command(c) if c.is(APPLY_BINDINGS) => ctx.set_handled(),
            _ => {
                self.contained.event(ctx, event, data, env);
            }
        };

        // Changes that occurred just now
        self.binding.append_change_required(
            self.contained.wrapped(),
            data,
            &mut self.pending_change,
        );
        if let Some(change) = self.pending_change.take() {
            self.binding
                .apply_change_to_data(self.contained.wrapped(), data, change, ctx)
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.contained.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        log::info!("Update called on BindingHost");
        if !old_data.same(data) {
            log::info!("Applied data to controlled widget ");
            self.binding
                .apply_data_to_controlled(data, self.contained.wrapped_mut(), ctx);
        }
        self.contained.update(ctx, old_data, data, env);
        self.binding.append_change_required(
            self.contained.wrapped(),
            data,
            &mut self.pending_change,
        );
        if self.pending_change.is_some() {
            ctx.submit_command(APPLY_BINDINGS, ctx.widget_state.id);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        let size = self.contained.layout(ctx, bc, data, env);
        self.binding.append_change_required(
            self.contained.wrapped(),
            data,
            &mut self.pending_change,
        );
        if self.pending_change.is_some() {
            ctx.submit_command(APPLY_BINDINGS, ctx.widget_state.id);
        }
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.contained.paint(ctx, data, env);
        // Can't submit commands from here...
        // No point pending it yet
        // have to assume that any bound state will get picked up later
    }
}
