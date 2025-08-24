use std::ops::IndexMut;
use std::ops::Index;

pub struct RingBuffer<T: Default> {
	capacity: usize,
	current_pos: usize,
	buffer: Vec<T>
}

impl<T: Default + Clone> RingBuffer<T> {
	pub fn new(capacity: usize) -> Self {
		Self {
			capacity,
			current_pos: 0,
			buffer: vec![T::default(); capacity]
		}
	}

	pub fn extend_defaults(&mut self, len: usize) -> bool {
		if self.capacity() < len {
			return false
		}

		for _ in 0..len {
			self.push(T::default());
		}

		true
	}
}

impl<T: Default> RingBuffer<T> {
	pub fn capacity(&self) -> usize {
		self.capacity
	}

	pub fn push(&mut self, value: T) {
		self.buffer[self.current_pos] = value;
		self.current_pos = (self.current_pos + 1) % self.capacity;
	}
}

impl<T: Default> Index<usize> for RingBuffer<T> {
	type Output = T;

	fn index(&self, idx: usize) -> &T {
		&self.buffer[(idx + self.current_pos) % self.capacity]
	}
}

impl<T: Default> Index<isize> for RingBuffer<T> {
	type Output = T;

	fn index(&self, idx: isize) -> &T {
		let mut idx = idx % self.capacity as isize;
		if idx < 0 {
			idx += self.capacity as isize;
		}
		&self[idx as usize]
	}
}

impl<T: Default> IndexMut<usize> for RingBuffer<T> {
	fn index_mut(&mut self, idx: usize) -> &mut T {
		&mut self.buffer[(idx + self.current_pos) % self.capacity]
	}
}

impl<T: Default> IndexMut<isize> for RingBuffer<T> {
	fn index_mut(&mut self, idx: isize) -> &mut T {
		let mut idx = idx % self.capacity as isize;
		if idx < 0 {
			idx += self.capacity as isize;
		}
		&mut self[idx as usize]
	}
}