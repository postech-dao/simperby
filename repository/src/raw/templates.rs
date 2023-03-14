use super::*;

pub(crate) async fn helper_0<R: Send + Sync + 'static>(
    s: &RawRepository,
    f: impl Fn(&RawRepositoryInner) -> R + Send + 'static,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_0_mut<R: Send + Sync + 'static>(
    s: &mut RawRepository,
    f: impl Fn(&mut RawRepositoryInner) -> R + Send + 'static,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_1<T1: Send + Sync + 'static + Clone, R: Send + Sync + 'static>(
    s: &RawRepository,
    f: impl Fn(&RawRepositoryInner, T1) -> R + Send + 'static,
    a1: T1,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner, a1), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_1_mut<T1: Send + Sync + 'static + Clone, R: Send + Sync + 'static>(
    s: &mut RawRepository,
    f: impl Fn(&mut RawRepositoryInner, T1) -> R + Send + 'static,
    a1: T1,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner, a1), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_2<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &RawRepository,
    f: impl Fn(&RawRepositoryInner, T1, T2) -> R + Send + 'static,
    a1: T1,
    a2: T2,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner, a1, a2), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_2_mut<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &mut RawRepository,
    f: impl Fn(&mut RawRepositoryInner, T1, T2) -> R + Send + 'static,
    a1: T1,
    a2: T2,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner, a1, a2), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_3<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    T3: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &RawRepository,
    f: impl Fn(&RawRepositoryInner, T1, T2, T3) -> R + Send + 'static,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let mut lock = s.inner.lock().await;
    let inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&inner, a1, a2, a3), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_3_mut<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    T3: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &mut RawRepository,
    f: impl Fn(&mut RawRepositoryInner, T1, T2, T3) -> R + Send + 'static,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) = tokio::task::spawn_blocking(move || (f(&mut inner, a1, a2, a3), inner))
        .await
        .unwrap();
    lock.replace(inner);
    result
}

pub(crate) async fn helper_5_mut<
    T1: Send + Sync + 'static + Clone,
    T2: Send + Sync + 'static + Clone,
    T3: Send + Sync + 'static + Clone,
    T4: Send + Sync + 'static + Clone,
    T5: Send + Sync + 'static + Clone,
    R: Send + Sync + 'static,
>(
    s: &mut RawRepository,
    f: impl Fn(&mut RawRepositoryInner, T1, T2, T3, T4, T5) -> R + Send + 'static,
    a1: T1,
    a2: T2,
    a3: T3,
    a4: T4,
    a5: T5,
) -> R {
    let mut lock = s.inner.lock().await;
    let mut inner = lock.take().expect("RawRepoImpl invariant violated");
    let (result, inner) =
        tokio::task::spawn_blocking(move || (f(&mut inner, a1, a2, a3, a4, a5), inner))
            .await
            .unwrap();
    lock.replace(inner);
    result
}
