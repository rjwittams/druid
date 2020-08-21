use crate::kurbo::{Point, Rect};
use crate::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx,
    Size, UpdateCtx, Widget, WidgetPod,
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

pub struct Scope<SP: ScopePolicy, W: Widget<SP::State>> {
    scope_policy: SP,
    state: Option<SP::State>,
    inner: WidgetPod<SP::State, W>,
    widget_added: bool,
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Scope<SP, W> {
    pub fn new(scope_policy: SP, inner: W) -> Self {
        Scope {
            scope_policy,
            state: None,
            inner: WidgetPod::new(inner),
            widget_added: false,
        }
    }

    fn ensure_state(&mut self, data: &SP::In) {
        match &mut self.state {
            Some(state) => self.scope_policy.replace_in_state(state, data),
            None => self.state = Some(self.scope_policy.default_state(data)),
        }
    }

    fn with_state<V>(
        &mut self,
        data: &SP::In,
        mut f: impl FnMut(&SP::State, &mut WidgetPod<SP::State, W>) -> V,
    ) -> V {
        self.ensure_state(data);
        f(self.state.as_ref().unwrap(), &mut self.inner)
    }

    fn with_state_mut<V>(
        &mut self,
        data: &SP::In,
        mut f: impl FnMut(&mut SP::State, &mut WidgetPod<SP::State, W>) -> V,
    ) -> V {
        self.ensure_state(data);
        f(self.state.as_mut().unwrap(), &mut self.inner)
    }

    fn write_back_input(&mut self, data: &mut SP::In) {
        if let Some(state) = &self.state {
            self.scope_policy.write_back_input(state, data);
        }
    }
}

impl<SP: ScopePolicy, W: Widget<SP::State>> Widget<SP::In> for Scope<SP, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut SP::In, env: &Env) {
        if self.widget_added {
            self.with_state_mut(data, |state, inner| inner.event(ctx, event, state, env));
            self.write_back_input(data);
            ctx.request_update()
        } else {
            log::info!("Scope dropping event {:?}", event);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &SP::In, env: &Env) {
        self.with_state(data, |state, inner| inner.lifecycle(ctx, event, state, env));
        if let LifeCycle::WidgetAdded = event {
            self.widget_added = true;
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &SP::In, data: &SP::In, env: &Env) {
        self.with_state(data, |state, inner| {
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
        self.with_state(data, |state, inner| {
            let size = inner.layout(ctx, bc, state, env);
            inner.set_layout_rect(ctx, state, env, Rect::from_origin_size(Point::ORIGIN, size));
            size
        })
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &SP::In, env: &Env) {
        self.with_state(data, |state, inner| inner.paint_raw(ctx, state, env));
    }
}
