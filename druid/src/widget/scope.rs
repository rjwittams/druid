use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx,
    Size, UpdateCtx, Widget,
};
use std::marker::PhantomData;

pub trait ScopePolicy {
    type In: Data;
    type State: Data;

    // Make a new state from the input
    fn default_state(&self, inner: &Self::In) -> Self::State;
    // Replace the input we have with a new one from outside
    fn replace_in_state(&self, state: &mut Self::State, inner: &Self::In);
    // Take the modifications we have made and write them back
    // to our input.
    fn write_back_input(&self, state: &Self::State, inner: &mut Self::In);
}

pub struct DefaultScopePolicy<F: Fn(In) -> State, L: Lens<State, In>, In, State> {
    make_state: F,
    lens: L,
    phantom_in: PhantomData<In>,
    phantom_state: PhantomData<State>,
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In, State> DefaultScopePolicy<F, L, In, State> {
    pub fn new(make_state: F, lens: L) -> Self {
        DefaultScopePolicy {
            make_state,
            lens,
            phantom_in: Default::default(),
            phantom_state: Default::default(),
        }
    }
}

impl<F: Fn(In) -> State, L: Lens<State, In>, In: Data, State: Data> ScopePolicy
    for DefaultScopePolicy<F, L, In, State>
{
    type In = In;
    type State = State;

    fn default_state(&self, inner: &In) -> State {
        (self.make_state)(inner.clone())
    }
    fn replace_in_state(&self, state: &mut State, data: &In) {
        self.lens.with_mut(state, |inner| {
            if !inner.same(&data) {
                *inner = data.clone()
            }
        });
    }
    fn write_back_input(&self, state: &State, data: &mut In) {
        self.lens.with(state, |inner| {
            if !inner.same(&data) {
                *data = inner.clone();
            }
        })
    }
}

struct StateHolder<SP: ScopePolicy> {
    state: Option<SP::State>,
    old_state: Option<SP::State>,
    scope_policy: SP,
}

impl<SP: ScopePolicy> StateHolder<SP> {
    pub fn new(state_access: SP) -> Self {
        StateHolder {
            state: None,
            old_state: None,
            scope_policy: state_access,
        }
    }

    fn ensure_state(&mut self, data: &SP::In) {
        match &mut self.state {
            Some(state) => self.scope_policy.replace_in_state(state, data),
            None => self.state = Some(self.scope_policy.default_state(data)),
        }
    }

    fn with_state<V>(&mut self, data: &SP::In, mut f: impl FnMut(&SP::State) -> V) -> V {
        self.ensure_state(data);
        f(self.state.as_ref().unwrap())
    }

    fn call_if_state_changed(&mut self, data: &SP::In, mut f: impl FnMut(&SP::State, &SP::State)) {
        self.ensure_state(data);
        let state = self.state.as_ref().unwrap();
        match &mut self.old_state {
            Some(os) => {
                if !os.same(state) {
                    f(os, state);
                    *os = state.clone()
                }
            }
            None => {
                let temp_old_state = state.clone();
                f(&temp_old_state, state);
                self.old_state = Some(temp_old_state)
            }
        }
    }

    fn with_state_mut<V>(&mut self, data: &SP::In, mut f: impl FnMut(&mut SP::State) -> V) -> V {
        self.ensure_state(data);
        f(self.state.as_mut().unwrap())
    }

    fn write_back_input(&mut self, data: &mut SP::In) {
        if let Some(state) = &self.state {
            self.scope_policy.write_back_input(state, data);
        }
    }
}

pub struct Scope<SP: ScopePolicy, W: Widget<SP::State>> {
    sh: StateHolder<SP>, // These parts are bundled away from inner in order to help the borrow checker.
    inner: W,
    widget_added: bool,
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Scope<SP, W> {
    pub fn new(state_access: SP, inner: W) -> Self {
        Scope {
            sh: StateHolder::new(state_access),
            inner,
            widget_added: false,
        }
    }
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Widget<SP::In> for Scope<SP, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut SP::In, env: &Env) {
        if self.widget_added {
            let holder = &mut self.sh;
            let inner = &mut self.inner;
            holder.with_state_mut(data, |state| inner.event(ctx, event, state, env));
            holder.write_back_input(data);
            ctx.request_update()
        } else {
            log::info!("Scope dropping event {:?}", event);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &SP::In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.lifecycle(ctx, event, state, env));
        if let LifeCycle::WidgetAdded = event {
            self.widget_added = true;
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &SP::In, data: &SP::In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;

        holder.call_if_state_changed(data, |old_state, state| {
            inner.update(ctx, &old_state, state, env)
        });
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &SP::In,
        env: &Env,
    ) -> Size {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.layout(ctx, bc, state, env))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &SP::In, env: &Env) {
        let holder = &mut self.sh;
        let inner = &mut self.inner;
        holder.with_state(data, |state| inner.paint(ctx, state, env));
    }
}
