use crate::wgpu::async_buffer::AsyncBuffer;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wgpu::{BufferDescriptor, Device, Maintain};

#[derive(Clone, Debug)]
pub struct AsyncDevice {
    device: Arc<Device>,
}

impl AsyncDevice {
    pub fn new(device: Device) -> Self {
        Self {
            device: Arc::new(device),
        }
    }

    pub async fn do_async<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Box<dyn FnOnce(R) -> () + Send + 'static>) -> (),
    {
        let future = WgpuFuture::new(self.device.clone());
        f(future.callback());
        return future.await;
    }

    pub fn create_buffer(self: Arc<Self>, desc: &BufferDescriptor) -> AsyncBuffer {
        let buffer = self.device.create_buffer(desc);
        AsyncBuffer::new(self.clone(), buffer)
    }
}

impl AsRef<Device> for AsyncDevice {
    fn as_ref(&self) -> &Device {
        return self.device.as_ref();
    }
}

struct WgpuFutureSharedState<T> {
    result: Option<T>,
    waker: Option<Waker>,
}

struct WgpuFuture<T> {
    device: Arc<Device>,
    state: Arc<Mutex<WgpuFutureSharedState<T>>>,
}

impl<T: Send + 'static> WgpuFuture<T> {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            state: Arc::new(Mutex::new(WgpuFutureSharedState {
                result: None,
                waker: None,
            })),
        }
    }

    /// Generates a callback function for this future that wakes the waker and sets the shared state
    pub fn callback(&self) -> Box<dyn FnOnce(T) -> () + Send + 'static> {
        let shared_state = self.state.clone();
        return Box::new(move |res: T| {
            let mut lock = shared_state
                .lock()
                .expect("wgpu future was poisoned on complete");
            let shared_state = lock.deref_mut();
            shared_state.result = Some(res);

            if let Some(waker) = shared_state.waker.take() {
                waker.wake()
            }
        });
    }
}

impl<T> Future for WgpuFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Poll whenever we enter to see if we can avoid waiting altogether
        self.device.poll(Maintain::Poll);

        // Check with scoped lock
        {
            let mut lock = self.state.lock().expect("wgpu future was poisoned on poll");

            if let Some(res) = lock.result.take() {
                return Poll::Ready(res);
            }

            lock.waker = Some(cx.waker().clone());
        }

        // Treat as green thread - we pass back but are happy to sit in a spin loop and poll
        cx.waker().wake_by_ref();

        return Poll::Pending;
    }
}
