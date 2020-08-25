use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx,
    Selector, Size, UpdateCtx, Widget,
};
use std::marker::PhantomData;

/// This trait indicates that a class is a wrapper of another widget that may have API you wish to access.
/// Used by BindingHost to "reach inside" things like LensWrapped in order to find the right widget to control,
/// given the bindings that it has.
///
/// Widgets that expect to be "bound" should implement this trait and return themselves.
/// Widgets that wrap another widget and do not provide anything likely to need binding should
/// recurse and call the same method on their inner widget.
///
/// This scheme isn't perfect as it stops at the first Bindable in all cases. However it covers the common case of
/// binding something that is already lensed
pub trait BindableAccess {
    type Wrapped;
    fn bindable(&self) -> &Self::Wrapped;
    fn bindable_mut(&mut self) -> &mut Self::Wrapped;
}

///  Its not possible to provide default impls of these traits because of the type parameters on Widgets
///  So we have a marker trait Bindable,
///  and for now the wrappers need their own implementations (to access their inner widget)
pub trait Bindable {}

impl<B: Bindable> BindableAccess for B {
    type Wrapped = Self;

    fn bindable(&self) -> &Self::Wrapped {
        self
    }

    fn bindable_mut(&mut self) -> &mut Self::Wrapped {
        self
    }
}

/// This is a two way binding between some data, and something it is controlling.
/// Usually this will be synchronising one bit of information in each,
/// eg one field of T bound to one 'property' of a controlled Widget.
pub trait Binding<T, Controlled> {
    /// The type of built up change from internal widget state that needs to be applied to the data.
    type Change;

    /// Take the bound value from the data T and apply it to the controlled item (usually a widget)
    /// This will occur during update, as that is when data has changed.
    fn apply_data_to_controlled(
        &self,
        data: &T,
        controlled: &mut Controlled,
        ctx: &mut UpdateCtx,
        env: &Env,
    );

    /// Mutate the passed in Change option to indicate to the BindingHost that an update to the data will be needed.
    /// This could get called from any Widget method in BindingHost, and allows changes to data to be queued up for the next event.
    /// This has no ctxt argument, because there is no common trait between contexts
    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
        env: &Env,
    );

    /// This will take the built up Change from internal widget state, and apply it to the data.
    /// If it is possible to read that change directly from the controlled item, the Change type can be () and the Some-ness of it alone
    /// will be enough to trigger the change to be applied.
    fn apply_change_to_data(
        &self,
        controlled: &Controlled,
        data: &mut T,
        change: Self::Change,
        ctx: &mut EventCtx,
        env: &Env,
    );
}

/// Allows a cons-list (or HList) of bindings to be built up, by treating a tuple of bindings as a binding.
impl<T, Controlled, Bind1: Binding<T, Controlled>, Bind2: Binding<T, Controlled>>
    Binding<T, Controlled> for (Bind1, Bind2)
{
    type Change = (Option<Bind1::Change>, Option<Bind2::Change>);

    fn apply_data_to_controlled(
        &self,
        data: &T,
        controlled: &mut Controlled,
        ctx: &mut UpdateCtx,
        env: &Env,
    ) {
        self.0.apply_data_to_controlled(data, controlled, ctx, env);
        self.1.apply_data_to_controlled(data, controlled, ctx, env);
    }

    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
        env: &Env,
    ) {
        let (change0, change1) = change.get_or_insert_with(|| (None, None));
        self.0
            .append_change_required(controlled, data, change0, env);
        self.1
            .append_change_required(controlled, data, change1, env);
        if let Some((None, None)) = change {
            *change = None;
        }
    }

    fn apply_change_to_data(
        &self,
        controlled: &Controlled,
        data: &mut T,
        change: Self::Change,
        ctx: &mut EventCtx,
        env: &Env,
    ) {
        let (change0, change1) = change;

        if let Some(change0) = change0 {
            self.0
                .apply_change_to_data(controlled, data, change0, ctx, env);
        }

        if let Some(change1) = change1 {
            self.1
                .apply_change_to_data(controlled, data, change1, ctx, env);
        }
    }
}

/// One way binding wrappers
pub struct DataToWidgetOnlyBinding<B>(pub B);

impl<T, Controlled, B: Binding<T, Controlled>> Binding<T, Controlled>
    for DataToWidgetOnlyBinding<B>
{
    type Change = ();

    fn apply_data_to_controlled(
        &self,
        data: &T,
        controlled: &mut Controlled,
        ctx: &mut UpdateCtx,
        env: &Env,
    ) {
        self.0.apply_data_to_controlled(data, controlled, ctx, env);
    }

    fn append_change_required(
        &self,
        _controlled: &Controlled,
        _data: &T,
        _change: &mut Option<Self::Change>,
        _env: &Env,
    ) {
    }

    fn apply_change_to_data(
        &self,
        _controlled: &Controlled,
        _data: &mut T,
        _change: Self::Change,
        _ctx: &mut EventCtx,
        _env: &Env,
    ) {
    }
}

pub struct WidgetToDataOnlyBinding<B>(B);

impl<T, Controlled, B: Binding<T, Controlled>> Binding<T, Controlled>
    for WidgetToDataOnlyBinding<B>
{
    type Change = B::Change;

    fn apply_data_to_controlled(
        &self,
        _data: &T,
        _controlled: &mut Controlled,
        _ctx: &mut UpdateCtx,
        _env: &Env,
    ) {
    }

    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
        env: &Env,
    ) {
        self.0.append_change_required(controlled, data, change, env);
    }

    fn apply_change_to_data(
        &self,
        controlled: &Controlled,
        data: &mut T,
        change: Self::Change,
        ctx: &mut EventCtx,
        env: &Env,
    ) {
        self.0
            .apply_change_to_data(controlled, data, change, ctx, env);
    }
}

/// This binds two lenses that evaluate to the same type together.
pub struct LensBinding<
    T,
    Controlled,
    PropValue,
    LT: Lens<T, PropValue>,
    LC: Lens<Controlled, PropValue>,
> {
    lens_from_data: LT,
    lens_from_controlled: LC,
    phantom_t: PhantomData<T>,
    phantom_c: PhantomData<Controlled>,
    phantom_p: PhantomData<PropValue>,
}

impl<T, Controlled, PropValue, LT: Lens<T, PropValue>, LC: Lens<Controlled, PropValue>>
    LensBinding<T, Controlled, PropValue, LT, LC>
{
    pub fn new(lens_from_data: LT, lens_from_controlled: LC) -> Self {
        LensBinding {
            lens_from_data,
            lens_from_controlled,
            phantom_t: Default::default(),
            phantom_c: Default::default(),
            phantom_p: Default::default(),
        }
    }
}

impl<T, Controlled, PropValue: Data, LT: Lens<T, PropValue>, LC: Lens<Controlled, PropValue>>
    Binding<T, Controlled> for LensBinding<T, Controlled, PropValue, LT, LC>
{
    type Change = PropValue;

    fn apply_data_to_controlled(
        &self,
        data: &T,
        controlled: &mut Controlled,
        ctx: &mut UpdateCtx,
        _env: &Env,
    ) {
        self.lens_from_data.with(data, |field_val| {
            self.lens_from_controlled.with_mut(controlled, |c_prop| {
                *c_prop = field_val.clone();
            })
        });
        // This is because we don't know anything about the dependencies - what needs to happen.
        // Could be passed in via constructor.
        ctx.request_paint();
    }

    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
        _env: &Env,
    ) {
        self.lens_from_data.with(data, |field_val| {
            self.lens_from_controlled.with(controlled, |c_prop| {
                *change = if c_prop.same(field_val) {
                    None
                } else {
                    Some(c_prop.clone())
                };
            })
        })
    }

    fn apply_change_to_data(
        &self,
        _controlled: &Controlled,
        data: &mut T,
        change: Self::Change,
        _ctx: &mut EventCtx,
        _env: &Env,
    ) {
        self.lens_from_data.with_mut(data, |field| *field = change)
    }
}

pub trait BindableProperty {
    type Controlling;
    type Value;
    type Change;

    fn write_prop(
        &self,
        controlled: &mut Self::Controlling,
        ctx: &mut UpdateCtx,
        field_val: &Self::Value,
        env: &Env,
    );
    fn append_changes(
        &self,
        controlled: &Self::Controlling,
        field_val: &Self::Value,
        change: &mut Option<Self::Change>,
        env: &Env,
    );
    fn update_data_from_change(
        &self,
        controlled: &Self::Controlling,
        ctx: &EventCtx,
        field: &mut Self::Value,
        change: Self::Change,
        env: &Env,
    );
}

pub struct LensPropBinding<
    T,
    Controlled,
    PropValue,
    LT: Lens<T, PropValue>,
    PropC: BindableProperty<Controlling = Controlled, Value = PropValue>,
> {
    lens_from_data: LT,
    prop_from_controlled: PropC,
    phantom_t: PhantomData<T>,
    phantom_c: PhantomData<Controlled>,
    phantom_p: PhantomData<PropValue>,
}

impl<
        T,
        Controlled,
        PropValue,
        LT: Lens<T, PropValue>,
        PropC: BindableProperty<Controlling = Controlled, Value = PropValue>,
    > LensPropBinding<T, Controlled, PropValue, LT, PropC>
{
    pub fn new(lens_from_data: LT, prop_from_controlled: PropC) -> Self {
        LensPropBinding {
            lens_from_data,
            prop_from_controlled,
            phantom_t: Default::default(),
            phantom_c: Default::default(),
            phantom_p: Default::default(),
        }
    }
}

impl<
        T,
        Controlled,
        PropValue,
        LT: Lens<T, PropValue>,
        PropC: BindableProperty<Controlling = Controlled, Value = PropValue>,
    > Binding<T, Controlled> for LensPropBinding<T, Controlled, PropValue, LT, PropC>
{
    type Change = PropC::Change;

    fn apply_data_to_controlled(
        &self,
        data: &T,
        controlled: &mut Controlled,
        ctx: &mut UpdateCtx,
        env: &Env,
    ) {
        self.lens_from_data.with(data, |field_val| {
            self.prop_from_controlled
                .write_prop(controlled, ctx, field_val, env)
        });
    }

    fn append_change_required(
        &self,
        controlled: &Controlled,
        data: &T,
        change: &mut Option<Self::Change>,
        env: &Env,
    ) {
        self.lens_from_data.with(data, |field_val| {
            self.prop_from_controlled
                .append_changes(controlled, field_val, change, env)
        })
    }

    fn apply_change_to_data(
        &self,
        controlled: &Controlled,
        data: &mut T,
        change: Self::Change,
        ctx: &mut EventCtx,
        env: &Env,
    ) {
        self.lens_from_data.with_mut(data, |field| {
            self.prop_from_controlled
                .update_data_from_change(controlled, ctx, field, change, env)
        })
    }
}

/// This series of traits provides combinators for building up bindings
pub trait LensBindingExt<T, U>: Lens<T, U> + Sized {
    // Need GATs to merge these methods

    fn bind_lens<C, L: Lens<C, U>>(self, other: L) -> LensBinding<T, C, U, Self, L> {
        LensBinding::new(self, other)
    }

    fn bind<BP: BindableProperty<Value = U>>(
        self,
        prop: BP,
    ) -> LensPropBinding<T, BP::Controlling, U, Self, BP> {
        LensPropBinding::new(self, prop)
    }
}

impl<T, U, M: Lens<T, U> + Sized + 'static> LensBindingExt<T, U> for M {}

pub trait WidgetBindingExt<T, U>: Widget<T> + Sized + BindableAccess
where
    Self::Wrapped: Widget<U>,
{
    fn binding<B: Binding<T, Self::Wrapped>>(
        self,
        binding: B,
    ) -> BindingHost<T, U, Self, Self::Wrapped, B> {
        BindingHost::new(self, binding)
    }
}

impl<T, U, W> WidgetBindingExt<T, U> for W
where
    W: Widget<T> + Sized + BindableAccess,
    Self::Wrapped: Widget<U>,
{
}

pub trait BindingExt<T, Controlled>: Binding<T, Controlled> + Sized {
    fn and<B: Binding<T, Controlled>>(self, other: B) -> (Self, B) {
        (self, other)
    }
    fn back(self) -> WidgetToDataOnlyBinding<Self> {
        WidgetToDataOnlyBinding(self)
    }
    fn forward(self) -> DataToWidgetOnlyBinding<Self> {
        DataToWidgetOnlyBinding(self)
    }
}

impl<T, Controlled, B: Binding<T, Controlled> + Sized> BindingExt<T, Controlled> for B {}

/// A binding host wraps a BindableAccess, and offers bindings from the Data at this stage of the hierarchy
/// to properties on that Bindable.
pub struct BindingHost<
    T,
    U,
    Contained: BindableAccess<Wrapped = Controlled> + Widget<T>,
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
        Contained: BindableAccess<Wrapped = Controlled> + Widget<T>,
        Controlled: Widget<U>,
        B: Binding<T, Controlled>,
    > BindingHost<T, U, Contained, Controlled, B>
{
    pub fn new(contained: Contained, binding: B) -> Self {
        BindingHost {
            contained,
            binding,
            pending_change: None,
            phantom_u: Default::default(),
        }
    }

    fn apply_pending_changes(&mut self, ctx: &mut EventCtx, data: &mut T, env: &Env) {
        if let Some(change) = self.pending_change.take() {
            self.binding
                .apply_change_to_data(self.contained.bindable(), data, change, ctx, env)
        }
    }

    fn check_for_changes(&mut self, data: &T, env: &Env) -> bool {
        self.binding.append_change_required(
            self.contained.bindable(),
            data,
            &mut self.pending_change,
            env,
        );
        self.pending_change.is_some()
    }
}

/// This command is sent to self trigger event to run - which is where data can be modified.
const APPLY_BINDINGS: Selector = Selector::new("druid-builtin.apply-bindings");

impl<
        T: Data,
        U,
        Contained: BindableAccess<Wrapped = Controlled> + Widget<T>,
        Controlled: Widget<U>,
        B: Binding<T, Controlled>,
    > Widget<T> for BindingHost<T, U, Contained, Controlled, B>
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        // Changes that occurred in other methods
        self.apply_pending_changes(ctx, data, env);

        match event {
            Event::Command(c) if c.is(APPLY_BINDINGS) => ctx.set_handled(), // We have handled this above
            _ => {
                self.contained.event(ctx, event, data, env);
            }
        };

        // Changes that occurred just now
        if self.check_for_changes(data, env) {
            self.apply_pending_changes(ctx, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.contained.lifecycle(ctx, event, data, env);
        // This can't be factored out as there is no common trait between contexts
        if self.check_for_changes(data, env) {
            ctx.submit_command(APPLY_BINDINGS, ctx.widget_state.id);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        if !old_data.same(data) {
            self.binding
                .apply_data_to_controlled(data, self.contained.bindable_mut(), ctx, env);
        }
        self.contained.update(ctx, old_data, data, env);
        if self.check_for_changes(data, env) {
            ctx.submit_command(APPLY_BINDINGS, ctx.widget_state.id);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        let size = self.contained.layout(ctx, bc, data, env);
        if self.check_for_changes(data, env) {
            ctx.submit_command(APPLY_BINDINGS, ctx.widget_state.id);
        }
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.contained.paint(ctx, data, env);
        // Can't submit commands from here currently.
        // No point pending it yet
        // have to assume that any bound state will get picked up later
    }
}
