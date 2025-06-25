use std::{cell::UnsafeCell, sync::Arc};

use anyhow::Result;
use tokio::sync::{Semaphore, SemaphorePermit};

pub struct UnsafeMutex<T: Sized + Send> {
	value: UnsafeCell<Arc<T>>,
	locker: Semaphore,
}

pub struct DataWrite<'a, T: Sized + Send> {
	pub value: *mut Arc<T>,
	_locked: SemaphorePermit<'a>,
}

impl<T> UnsafeMutex<T>
where
	T: Send,
{
	pub fn new(value: T) -> Self {
		Self {
			value: UnsafeCell::new(Arc::new(value)),
			locker: Semaphore::new(1),
		}
	}

	pub fn read(&self) -> Arc<T> {
		unsafe {
			let value = self.value.get();
			(*value).clone()
		}
	}

	pub async fn write<'a>(&'a self) -> Result<DataWrite<'a, T>> {
		let locked = self.locker.acquire().await?;

		Ok(DataWrite {
			value: self.value.get(),
			_locked: locked,
		})
	}
}

impl<T> UnsafeMutex<T>
where
	T: Send + Clone,
{
	pub fn read_imutable(&self) -> T {
		unsafe {
			let value = self.value.get();
			(*value).as_ref().clone()
		}
	}
}

impl<'a, T: Sized + Send> DataWrite<'a, T> {
	pub fn set(&self, value: T) {
		unsafe { *self.value = Arc::new(value) }
	}

	pub fn get(&self) -> &Arc<T> {
		let vb = unsafe { &*self.value };

		vb
	}
}

impl<'a, T: Sized + Send + Clone> DataWrite<'a, T> {
	pub fn update(&self, updater: impl FnOnce(T) -> T) {
		let old_value = self.get();
		let v = (**old_value).clone();
		let new_value = updater(v);

		unsafe {
			*self.value = Arc::new(new_value);
		}
	}

	pub fn get_imutable(&self) -> T {
		let vb = unsafe { &*self.value };

		vb.as_ref().clone()
	}
}

unsafe impl<T: Sized + Send> Sync for UnsafeMutex<T> {}
unsafe impl<T: Sized + Send> Send for UnsafeMutex<T> {}
