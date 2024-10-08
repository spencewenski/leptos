#[cfg(feature = "ssr")]
use super::MarkBranch;
use super::{
    add_attr::AddAnyAttr, Mountable, Position, PositionState, Render,
    RenderHtml, WrappedView,
};
use crate::{
    html::attribute::{
        any_attribute::{AnyAttribute, IntoAnyAttribute},
        Attribute,
    },
    hydration::Cursor,
    renderer::Renderer,
    ssr::StreamBuilder,
};
use std::{
    any::{Any, TypeId},
    fmt::Debug,
    marker::PhantomData,
};
#[cfg(feature = "ssr")]
use std::{future::Future, pin::Pin};

/// A type-erased view. This can be used if control flow requires that multiple different types of
/// view must be received, and it is either impossible or too cumbersome to use the `EitherOf___`
/// enums.
///
/// It can also be used to create recursive components, which otherwise cannot return themselves
/// due to the static typing of the view tree.
///
/// Generally speaking, using `AnyView` restricts the amount of information available to the
/// compiler and should be limited to situations in which it is necessary to preserve the maximum
/// amount of type information possible.
pub struct AnyView<R>
where
    R: Renderer,
{
    type_id: TypeId,
    value: Box<dyn Any + Send>,

    // The fields below are cfg-gated so they will not be included in WASM bundles if not needed.
    // Ordinarily, the compiler can simply omit this dead code because the methods are not called.
    // With this type-erased wrapper, however, the compiler is not *always* able to correctly
    // eliminate that code.
    #[cfg(feature = "ssr")]
    html_len: usize,
    #[cfg(feature = "ssr")]
    to_html: fn(Box<dyn Any>, &mut String, &mut Position, bool, bool),
    #[cfg(feature = "ssr")]
    to_html_async:
        fn(Box<dyn Any>, &mut StreamBuilder, &mut Position, bool, bool),
    #[cfg(feature = "ssr")]
    to_html_async_ooo:
        fn(Box<dyn Any>, &mut StreamBuilder, &mut Position, bool, bool),
    build: fn(Box<dyn Any>) -> AnyViewState<R>,
    rebuild: fn(TypeId, Box<dyn Any>, &mut AnyViewState<R>),
    add_any_attr: fn(Box<dyn Any>, AnyAttribute<R>) -> AnyView<R>,
    #[cfg(feature = "ssr")]
    #[allow(clippy::type_complexity)]
    resolve:
        fn(Box<dyn Any>) -> Pin<Box<dyn Future<Output = AnyView<R>> + Send>>,
    #[cfg(feature = "ssr")]
    dry_resolve: fn(&mut Box<dyn Any + Send>),
    #[cfg(feature = "hydrate")]
    #[cfg(feature = "hydrate")]
    #[allow(clippy::type_complexity)]
    hydrate_from_server:
        fn(Box<dyn Any>, &Cursor<R>, &PositionState) -> AnyViewState<R>,
}

/// Retained view state for [`AnyView`].
pub struct AnyViewState<R>
where
    R: Renderer,
{
    type_id: TypeId,
    state: Box<dyn Any>,
    unmount: fn(&mut dyn Any),
    mount: fn(&mut dyn Any, parent: &R::Element, marker: Option<&R::Node>),
    insert_before_this: fn(&dyn Any, child: &mut dyn Mountable<R>) -> bool,
    rndr: PhantomData<R>,
}

impl<R> Debug for AnyViewState<R>
where
    R: Renderer,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyViewState")
            .field("type_id", &self.type_id)
            .field("state", &self.state)
            .field("unmount", &self.unmount)
            .field("mount", &self.mount)
            .field("insert_before_this", &self.insert_before_this)
            .field("rndr", &self.rndr)
            .finish()
    }
}

/// Allows converting some view into [`AnyView`].
pub trait IntoAny<R>
where
    R: Renderer,
{
    /// Converts the view into a type-erased [`AnyView`].
    fn into_any(self) -> AnyView<R>;
}

fn mount_any<R, T>(
    state: &mut dyn Any,
    parent: &R::Element,
    marker: Option<&R::Node>,
) where
    T: Render<R>,
    T::State: 'static,
    R: Renderer,
{
    let state = state
        .downcast_mut::<T::State>()
        .expect("AnyViewState::as_mountable couldn't downcast state");
    state.mount(parent, marker)
}

fn unmount_any<R, T>(state: &mut dyn Any)
where
    T: Render<R>,
    T::State: 'static,
    R: Renderer,
{
    let state = state
        .downcast_mut::<T::State>()
        .expect("AnyViewState::unmount couldn't downcast state");
    state.unmount();
}

fn insert_before_this<R, T>(
    state: &dyn Any,
    child: &mut dyn Mountable<R>,
) -> bool
where
    T: Render<R>,
    T::State: 'static,
    R: Renderer + 'static,
{
    let state = state
        .downcast_ref::<T::State>()
        .expect("AnyViewState::insert_before_this couldn't downcast state");
    state.insert_before_this(child)
}

impl<T, R> IntoAny<R> for T
where
    T: Send,
    T: RenderHtml<R> + 'static,
    T::State: 'static,
    R: Renderer + 'static,
{
    // inlining allows the compiler to remove the unused functions
    // i.e., doesn't ship HTML-generating code that isn't used
    #[inline(always)]
    fn into_any(self) -> AnyView<R> {
        #[cfg(feature = "ssr")]
        let html_len = self.html_len();

        let value = Box::new(self) as Box<dyn Any + Send>;

        #[cfg(feature = "ssr")]
        let dry_resolve = |value: &mut Box<dyn Any + Send>| {
            let value = value
                .downcast_mut::<T>()
                .expect("AnyView::resolve could not be downcast");
            value.dry_resolve();
        };

        #[cfg(feature = "ssr")]
        let resolve = |value: Box<dyn Any>| {
            let value = value
                .downcast::<T>()
                .expect("AnyView::resolve could not be downcast");
            Box::pin(async move { value.resolve().await.into_any() })
                as Pin<Box<dyn Future<Output = AnyView<R>> + Send>>
        };
        #[cfg(feature = "ssr")]
        let to_html = |value: Box<dyn Any>,
                       buf: &mut String,
                       position: &mut Position,
                       escape: bool,
                       mark_branches: bool| {
            let type_id = mark_branches
                .then(|| format!("{:?}", TypeId::of::<T>()))
                .unwrap_or_default();
            let value = value
                .downcast::<T>()
                .expect("AnyView::to_html could not be downcast");
            if mark_branches {
                buf.open_branch(&type_id);
            }
            value.to_html_with_buf(buf, position, escape, mark_branches);
            if mark_branches {
                buf.close_branch(&type_id);
            }
        };
        #[cfg(feature = "ssr")]
        let to_html_async = |value: Box<dyn Any>,
                             buf: &mut StreamBuilder,
                             position: &mut Position,
                             escape: bool,
                             mark_branches: bool| {
            let type_id = mark_branches
                .then(|| format!("{:?}", TypeId::of::<T>()))
                .unwrap_or_default();
            let value = value
                .downcast::<T>()
                .expect("AnyView::to_html could not be downcast");
            if mark_branches {
                buf.open_branch(&type_id);
            }
            value.to_html_async_with_buf::<false>(
                buf,
                position,
                escape,
                mark_branches,
            );
            if mark_branches {
                buf.close_branch(&type_id);
            }
        };
        #[cfg(feature = "ssr")]
        let to_html_async_ooo =
            |value: Box<dyn Any>,
             buf: &mut StreamBuilder,
             position: &mut Position,
             escape: bool,
             mark_branches: bool| {
                let value = value
                    .downcast::<T>()
                    .expect("AnyView::to_html could not be downcast");
                value.to_html_async_with_buf::<true>(
                    buf,
                    position,
                    escape,
                    mark_branches,
                );
            };
        let build = |value: Box<dyn Any>| {
            let value = value
                .downcast::<T>()
                .expect("AnyView::build couldn't downcast");
            let state = Box::new(value.build());

            AnyViewState {
                type_id: TypeId::of::<T>(),
                state,
                rndr: PhantomData,
                mount: mount_any::<R, T>,
                unmount: unmount_any::<R, T>,
                insert_before_this: insert_before_this::<R, T>,
            }
        };
        #[cfg(feature = "hydrate")]
        let hydrate_from_server =
            |value: Box<dyn Any>,
             cursor: &Cursor<R>,
             position: &PositionState| {
                let value = value
                    .downcast::<T>()
                    .expect("AnyView::hydrate_from_server couldn't downcast");
                let state = Box::new(value.hydrate::<true>(cursor, position));

                AnyViewState {
                    type_id: TypeId::of::<T>(),
                    state,
                    rndr: PhantomData,
                    mount: mount_any::<R, T>,
                    unmount: unmount_any::<R, T>,
                    insert_before_this: insert_before_this::<R, T>,
                }
            };

        let rebuild = |new_type_id: TypeId,
                       value: Box<dyn Any>,
                       state: &mut AnyViewState<R>| {
            let value = value
                .downcast::<T>()
                .expect("AnyView::rebuild couldn't downcast value");
            if new_type_id == state.type_id {
                let state = state
                    .state
                    .downcast_mut()
                    .expect("AnyView::rebuild couldn't downcast state");
                value.rebuild(state);
            } else {
                let mut new = value.into_any().build();
                state.insert_before_this(&mut new);
                state.unmount();
                *state = new;
            }
        };

        let add_any_attr = |value: Box<dyn Any>, attr: AnyAttribute<R>| {
            let value = value
                .downcast::<T>()
                .expect("AnyView::add_any_attr() couldn't downcast value");
            value.add_any_attr(attr).into_any()
        };

        AnyView {
            type_id: TypeId::of::<T>(),
            value,
            build,
            rebuild,
            add_any_attr,
            #[cfg(feature = "ssr")]
            resolve,
            #[cfg(feature = "ssr")]
            dry_resolve,
            #[cfg(feature = "ssr")]
            html_len,
            #[cfg(feature = "ssr")]
            to_html,
            #[cfg(feature = "ssr")]
            to_html_async,
            #[cfg(feature = "ssr")]
            to_html_async_ooo,
            #[cfg(feature = "hydrate")]
            hydrate_from_server,
        }
    }
}

impl<R> Render<R> for AnyView<R>
where
    R: Renderer + 'static,
{
    type State = AnyViewState<R>;

    fn build(self) -> Self::State {
        (self.build)(self.value)
    }

    fn rebuild(self, state: &mut Self::State) {
        (self.rebuild)(self.type_id, self.value, state)
    }
}

impl<R> AddAnyAttr<R> for AnyView<R>
where
    R: Renderer + 'static,
{
    type Output<SomeNewAttr: Attribute<R>> = WrappedView<AnyView<R>>;

    fn add_any_attr<NewAttr: Attribute<R>>(
        self,
        attr: NewAttr,
    ) -> Self::Output<NewAttr>
    where
        Self::Output<NewAttr>: RenderHtml<R>,
    {
        let attr = attr.into_cloneable_owned().into_any_attr();
        let new = (self.add_any_attr)(self.value, attr).into_any();
        WrappedView::new(new)
    }
}

impl<R> RenderHtml<R> for AnyView<R>
where
    R: Renderer + 'static,
{
    type AsyncOutput = Self;

    fn dry_resolve(&mut self) {
        #[cfg(feature = "ssr")]
        {
            (self.dry_resolve)(&mut self.value)
        }
        #[cfg(not(feature = "ssr"))]
        panic!(
            "You are rendering AnyView to HTML without the `ssr` feature \
             enabled."
        );
    }

    async fn resolve(self) -> Self::AsyncOutput {
        #[cfg(feature = "ssr")]
        {
            (self.resolve)(self.value).await
        }
        #[cfg(not(feature = "ssr"))]
        panic!(
            "You are rendering AnyView to HTML without the `ssr` feature \
             enabled."
        );
    }

    const MIN_LENGTH: usize = 0;

    fn to_html_with_buf(
        self,
        buf: &mut String,
        position: &mut Position,
        escape: bool,
        mark_branches: bool,
    ) {
        #[cfg(feature = "ssr")]
        (self.to_html)(self.value, buf, position, escape, mark_branches);
        #[cfg(not(feature = "ssr"))]
        {
            _ = mark_branches;
            _ = buf;
            _ = position;
            _ = escape;
            panic!(
                "You are rendering AnyView to HTML without the `ssr` feature \
                 enabled."
            );
        }
    }

    fn to_html_async_with_buf<const OUT_OF_ORDER: bool>(
        self,
        buf: &mut StreamBuilder,
        position: &mut Position,
        escape: bool,
        mark_branches: bool,
    ) where
        Self: Sized,
    {
        #[cfg(feature = "ssr")]
        if OUT_OF_ORDER {
            (self.to_html_async_ooo)(
                self.value,
                buf,
                position,
                escape,
                mark_branches,
            );
        } else {
            (self.to_html_async)(
                self.value,
                buf,
                position,
                escape,
                mark_branches,
            );
        }
        #[cfg(not(feature = "ssr"))]
        {
            _ = buf;
            _ = position;
            _ = escape;
            _ = mark_branches;
            panic!(
                "You are rendering AnyView to HTML without the `ssr` feature \
                 enabled."
            );
        }
    }

    fn hydrate<const FROM_SERVER: bool>(
        self,
        cursor: &Cursor<R>,
        position: &PositionState,
    ) -> Self::State {
        #[cfg(feature = "hydrate")]
        if FROM_SERVER {
            (self.hydrate_from_server)(self.value, cursor, position)
        } else {
            panic!(
                "hydrating AnyView from inside a ViewTemplate is not \
                 supported."
            );
        }
        #[cfg(not(feature = "hydrate"))]
        {
            _ = cursor;
            _ = position;
            panic!(
                "You are trying to hydrate AnyView without the `hydrate` \
                 feature enabled."
            );
        }
    }

    fn html_len(&self) -> usize {
        #[cfg(feature = "ssr")]
        {
            self.html_len
        }
        #[cfg(not(feature = "ssr"))]
        {
            0
        }
    }
}

impl<R> Mountable<R> for AnyViewState<R>
where
    R: Renderer + 'static,
{
    fn unmount(&mut self) {
        (self.unmount)(&mut *self.state)
    }

    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        (self.mount)(&mut *self.state, parent, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        (self.insert_before_this)(&*self.state, child)
    }
}
/*
#[cfg(test)]
mod tests {
    use super::IntoAny;
    use crate::{
        html::element::{p, span},
        renderer::mock_dom::MockDom,
        view::{any_view::AnyView, RenderHtml},
    };

    #[test]
    fn should_handle_html_creation() {
        let x = 1;
        let mut buf = String::new();
        let view: AnyView<MockDom> = if x == 0 {
            p((), "foo").into_any()
        } else {
            span((), "bar").into_any()
        };
        view.to_html(&mut buf, &Default::default());
        assert_eq!(buf, "<span>bar</span><!>");
    }
}
 */
