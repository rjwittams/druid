use std::marker::PhantomData;

use crate::widget::prelude::*;
use crate::widget::WidgetWrapper;
use crate::{Data, Lens, Point, WidgetPod};

/// A policy that controls how a [`Scope`] will interact with its surrounding
/// application data. Specifically, how to create an initial State from the
/// input, and how to synchronise the two using a [`ScopeTransfer`].
///
/// [`Scope`]: struct.Scope.html
/// [`ScopeTransfer`]: trait.ScopeTransfer.html
pub trait ScopePolicy {
    /// The type of data that comes in from the surrounding application or scope.
    type In: Data;
    /// The type of data that the `Scope` will maintain internally.
    /// This will usually be larger than the input data, and will embed the input data.
    type State: Data;
    /// The type of transfer that will be used to synchronise internal and application state
    type Transfer: ScopeTransfer<In = Self::In, State = Self::State>;
    /// Make a new state and transfer from the input.
    ///
    /// This consumes the policy, so non-cloneable items can make their way
    /// into the state this way.
    fn create(self, inner: &Self::In, env: &Env) -> (Self::State, Self::Transfer);
}

/// A `ScopeTransfer` knows how to synchronise input data with its counterpart
/// within a [`Scope`].
///
/// It is separate from the policy mainly to allow easy use of lenses to do
/// synchronisation, with a custom [`ScopePolicy`].
///
/// [`Scope`]: struct.Scope.html
/// [`ScopePolicy`]: trait.ScopePolicy.html
pub trait ScopeTransfer {
    /// The type of data that comes in from the surrounding application or scope.
    type In: Data;
    /// The type of data that the Scope will maintain internally.
    type State: Data;

    /// Replace the input we have within our State with a new one from outside
    fn read_input(&self, state: &mut Self::State, input: &Self::In, env: &Env);
    /// Take the modifications we have made and write them back
    /// to our input.
    fn write_back_input(&self, state: &Self::State, input: &mut Self::In);

    /// Update any computed properties that have been invalidated by changes in the state.
    fn update_computed(&self, old_state: &Self::State, state: &mut Self::State, env: &Env) -> bool;
}

/// A default implementation of [`ScopePolicy`] that takes a function and a transfer.
///
/// [`ScopePolicy`]: trait.ScopePolicy.html
pub struct DefaultScopePolicy<F: FnOnce(Transfer::In) -> Transfer::State, Transfer: ScopeTransfer> {
    make_state: F,
    transfer: Transfer,
}

impl<F: FnOnce(Transfer::In) -> Transfer::State, Transfer: ScopeTransfer>
    DefaultScopePolicy<F, Transfer>
{
    /// Create a `ScopePolicy` from a factory function and a `ScopeTransfer`.
    pub fn new(make_state: F, transfer: Transfer) -> Self {
        DefaultScopePolicy {
            make_state,
            transfer,
        }
    }
}

impl<F: FnOnce(In) -> State, L: Lens<State, In>, In: Data, State: Data>
    DefaultScopePolicy<F, LensScopeTransfer<L, In, State>>
{
    /// Create a `ScopePolicy` from a factory function and a lens onto that
    /// `Scope`'s state.
    pub fn from_lens(make_state: F, lens: L) -> Self {
        Self::new(make_state, LensScopeTransfer::new(lens))
    }
}

impl<F: FnOnce(Transfer::In) -> Transfer::State, Transfer: ScopeTransfer> ScopePolicy
    for DefaultScopePolicy<F, Transfer>
{
    type In = Transfer::In;
    type State = Transfer::State;
    type Transfer = Transfer;

    fn create(self, input: &Self::In, _env: &Env) -> (Self::State, Self::Transfer) {
        let state = (self.make_state)(input.clone());
        (state, self.transfer)
    }
}

/// A `ScopeTransfer` that uses a Lens to synchronise between a large internal
/// state and a small input.
pub struct LensScopeTransfer<L: Lens<State, In>, In, State> {
    lens: L,
    phantom_in: PhantomData<In>,
    phantom_state: PhantomData<State>,
}

impl<L: Lens<State, In>, In, State> LensScopeTransfer<L, In, State> {
    /// Create a `ScopeTransfer` from a Lens onto a portion of the `Scope`'s state.
    pub fn new(lens: L) -> Self {
        LensScopeTransfer {
            lens,
            phantom_in: PhantomData::default(),
            phantom_state: PhantomData::default(),
        }
    }
}

/// A scope policy that is provided with some state on construction,
/// and does not synchronise it with external app data.
pub struct IsolatedScopePolicy<In, State> {
    state: Option<State>,
    phantom_i: PhantomData<*const In>,
}

impl<In, State> IsolatedScopePolicy<In, State> {
    /// Create an IsolatedScopePolicy with the provided data
    pub fn new(state: State) -> Self {
        IsolatedScopePolicy {
            state: state.into(),
            phantom_i: PhantomData,
        }
    }
}

impl<In: Data, State: Data> ScopePolicy for IsolatedScopePolicy<In, State> {
    type In = In;
    type State = State;
    type Transfer = Self;

    fn create(mut self, _inner: &Self::In, _env: &Env) -> (Self::State, Self::Transfer) {
        (self.state.take().unwrap(), self)
    }
}

impl<In: Data, State: Data> ScopeTransfer for IsolatedScopePolicy<In, State> {
    type In = In;
    type State = State;

    fn read_input(&self, _state: &mut Self::State, _input: &Self::In, _env: &Env) {}

    fn write_back_input(&self, _state: &Self::State, _input: &mut Self::In) {}

    fn update_computed(
        &self,
        _old_state: &Self::State,
        _state: &mut Self::State,
        _env: &Env,
    ) -> bool {
        false
    }
}

impl<L: Lens<State, In>, In: Data, State: Data> ScopeTransfer for LensScopeTransfer<L, In, State> {
    type In = In;
    type State = State;

    fn read_input(&self, state: &mut State, data: &In, _env: &Env) {
        self.lens.with_mut(state, |embedded_input| {
            if !embedded_input.same(&data) {
                *embedded_input = data.clone()
            }
        });
    }

    fn write_back_input(&self, state: &State, data: &mut In) {
        self.lens.with(state, |embedded_input| {
            if !embedded_input.same(&data) {
                *data = embedded_input.clone();
            }
        });
    }

    fn update_computed(
        &self,
        _old_state: &Self::State,
        _state: &mut Self::State,
        _env: &Env,
    ) -> bool {
        false
    }
}

enum ScopeContent<SP: ScopePolicy> {
    Policy {
        policy: Option<SP>,
    },
    Transfer {
        state: SP::State,
        transfer: SP::Transfer,
    },
}

/// A widget that allows encapsulation of application state.
///
/// This is useful in circumstances where
/// * A (potentially reusable) widget is composed of a tree of multiple cooperating child widgets
/// * Those widgets communicate amongst themselves using Druid's reactive data mechanisms
/// * It is undesirable to complicate the surrounding application state with the internal details
///   of the widget.
///
///
/// Examples include:
/// * In a tabs widget composed of a tab bar, and a widget switching body, those widgets need to
///   cooperate on which tab is selected. However not every user of a tabs widget wishes to
///   encumber their application state with this internal detail - especially as many tabs widgets may
///   reasonably exist in an involved application.
/// * In a table/grid widget composed of various internal widgets, many things need to be synchronised.
///   Scroll position, heading moves, drag operations, sort/filter operations. For many applications
///   access to this internal data outside of the table widget isn't needed.
///   For this reason it may be useful to use a Scope to establish private state.
///
/// A scope embeds some input state (from its surrounding application or parent scope)
/// into a larger piece of internal state. This is controlled by a user provided policy.
///
/// The ScopePolicy needs to do two things
/// a) Create a new scope from the initial value of its input,
/// b) Provide two way synchronisation between the input and the state via a ScopeTransfer
///
/// Convenience methods are provided to make a policy from a function and a lens.
/// It may sometimes be advisable to implement ScopePolicy directly if you need to
/// mention the type of a Scope.
///
/// # Examples
/// ```
/// use druid::{Data, Lens, WidgetExt};
/// use druid::widget::{TextBox, Scope};
/// #[derive(Clone, Data, Lens)]
/// struct AppState {
///     name: String,
/// }
///
/// #[derive(Clone, Data, Lens)]
/// struct PrivateState {
///     text: String,
///     other: u32,
/// }
///
/// impl PrivateState {
///     pub fn new(text: String) -> Self {
///         PrivateState { text, other: 0 }
///     }
/// }
///
/// fn main() {
///     let scope = Scope::from_lens(
///         PrivateState::new,
///         PrivateState::text,
///         TextBox::new().lens(PrivateState::text),
///     );
/// }
/// ```
pub struct Scope<SP: ScopePolicy, W: Widget<SP::State>> {
    content: ScopeContent<SP>,
    inner: WidgetPod<SP::State, W>,
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Scope<SP, W> {
    /// Create a new scope from a policy and an inner widget
    pub fn new(policy: SP, inner: W) -> Self {
        Scope {
            content: ScopeContent::Policy {
                policy: Some(policy),
            },
            inner: WidgetPod::new(inner),
        }
    }

    /// This allows you to access the content of the Scopes state from
    /// outside the widget.
    pub fn state(&self) -> Option<&SP::State> {
        if let ScopeContent::Transfer { ref state, .. } = &self.content {
            Some(state)
        } else {
            None
        }
    }

    /// This allows you to mutably access the content of the Scopes state from
    /// outside the widget.
    pub fn state_mut(&mut self) -> Option<&mut SP::State> {
        if let ScopeContent::Transfer { ref mut state, .. } = &mut self.content {
            Some(state)
        } else {
            None
        }
    }

    fn with_state<V>(
        &mut self,
        is_update: bool,
        data: &SP::In,
        env: &Env,
        mut f: impl FnMut(&SP::State, &mut WidgetPod<SP::State, W>) -> V,
    ) -> V {
        match &mut self.content {
            ScopeContent::Policy { policy } => {
                // We know that the policy is a Some - it is an option to allow
                // us to take ownership before replacing the content.
                let (state, transfer) = policy.take().unwrap().create(data, env);
                let v = f(&state, &mut self.inner);
                self.content = ScopeContent::Transfer { state, transfer };
                v
            }
            ScopeContent::Transfer {
                ref mut state,
                transfer,
            } => {
                if is_update {
                    transfer.read_input(state, data, env);
                    if let Some(old_state) = &self.inner.old_data {
                        transfer.update_computed(old_state, state, env);
                    }
                }
                f(state, &mut self.inner)
            }
        }
    }

    fn with_state_mut<V>(
        &mut self,
        data: &SP::In,
        env: &Env,
        mut f: impl FnMut(&mut SP::State, &mut WidgetPod<SP::State, W>) -> V,
    ) -> V {
        match &mut self.content {
            ScopeContent::Policy { policy } => {
                // We know that the policy is a Some - it is an option to allow
                // us to take ownership before replacing the content.
                let (mut state, transfer) = policy.take().unwrap().create(data, env);
                let v = f(&mut state, &mut self.inner);
                self.content = ScopeContent::Transfer { state, transfer };
                v
            }
            ScopeContent::Transfer {
                ref mut state,
                transfer: _,
            } => f(state, &mut self.inner),
        }
    }

    fn update_computed_and_write_back(&mut self, data: &mut SP::In, _env: &Env) -> bool {
        let inner = &mut self.inner;

        if let ScopeContent::Transfer { state, transfer } = &mut self.content {
            if let Some(old_state) = &inner.old_data {
                if !old_state.same(state) {
                    //transfer.update_computed(old_state, state, env);
                    transfer.write_back_input(state, data);
                    return true;
                }
            }
        }
        true
    }
}

impl<
        F: FnOnce(Transfer::In) -> Transfer::State,
        Transfer: ScopeTransfer,
        W: Widget<Transfer::State>,
    > Scope<DefaultScopePolicy<F, Transfer>, W>
{
    /// Create a new policy from a function creating the state, and a ScopeTransfer synchronising it
    pub fn from_function(make_state: F, transfer: Transfer, inner: W) -> Self {
        Self::new(DefaultScopePolicy::new(make_state, transfer), inner)
    }
}

impl<In: Data, State: Data, F: Fn(In) -> State, L: Lens<State, In>, W: Widget<State>>
    Scope<DefaultScopePolicy<F, LensScopeTransfer<L, In, State>>, W>
{
    /// Create a new policy from a function creating the state, and a Lens synchronising it
    pub fn from_lens(make_state: F, lens: L, inner: W) -> Self {
        Self::new(DefaultScopePolicy::from_lens(make_state, lens), inner)
    }
}

impl<In: Data, State: Data, W: Widget<State>> Scope<IsolatedScopePolicy<In, State>, W> {
    /// Create a scope from some static data. It will not synchronize with its surroundings
    pub fn isolate(state: State, widget: W) -> Self {
        Scope::new(IsolatedScopePolicy::new(state), widget)
    }
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Widget<SP::In> for Scope<SP, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut SP::In, env: &Env) {
        self.with_state_mut(data, env, |state, inner| {
            inner.event(ctx, event, state, env);
        });

        self.update_computed_and_write_back(data, env);
        ctx.request_update()
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &SP::In, env: &Env) {
        self.with_state(false, data, env, |state, inner| {
            inner.lifecycle(ctx, event, state, env)
        });
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &SP::In, data: &SP::In, env: &Env) {
        self.with_state(true, data, env, |state, inner| {
            inner.update(ctx, state, env)
        });
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &SP::In,
        env: &Env,
    ) -> Size {
        self.with_state(false, data, env, |state, inner| {
            let size = inner.layout(ctx, bc, state, env);
            inner.set_origin(ctx, state, env, Point::ORIGIN);
            size
        })
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &SP::In, env: &Env) {
        self.with_state(false, data, env, |state, inner| {
            inner.paint_raw(ctx, state, env)
        });
    }
}

impl<SP: ScopePolicy, W: Widget<SP::State>> WidgetWrapper for Scope<SP, W> {
    widget_wrapper_pod_body!(W, inner);
}
