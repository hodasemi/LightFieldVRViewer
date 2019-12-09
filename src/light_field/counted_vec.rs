// use std::slice::Iter;

#[derive(Debug, Clone)]
pub struct CountedValue<T: PartialEq + Copy> {
    count: usize,
    value: T,
}

#[derive(Debug, Clone)]
pub struct CountedVec<T: PartialEq + Copy> {
    data: Vec<CountedValue<T>>,
}

impl<T: PartialEq + Copy> CountedValue<T> {
    pub fn new(value: T) -> Self {
        CountedValue { count: 1, value }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }

    // pub fn decrement(&mut self) {
    //     self.count -= 1;
    // }

    // pub fn value_ref(&self) -> &T {
    //     &self.value
    // }

    // pub fn count(&self) -> usize {
    //     self.count
    // }
}

impl<T: PartialEq + Copy> CountedValue<T> {
    // pub fn value(&self) -> T {
    //     self.value
    // }
}

impl<T: PartialEq + Copy> CountedVec<T> {
    pub fn new() -> Self {
        CountedVec { data: Vec::new() }
    }

    pub fn insert(&mut self, value: T) {
        match self.data.iter().position(|v| v.value == value) {
            Some(position) => self.data[position].increment(),
            None => self.data.push(CountedValue::new(value)),
        }
    }

    // pub fn iter(&self) -> Iter<'_, CountedValue<T>> {
    //     self.data.iter()
    // }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    // pub fn total_count(&self) -> usize {
    //     let mut total_count = 0;

    //     for value in self.data.iter() {
    //         total_count += value.count;
    //     }

    //     total_count
    // }
}

impl<T: PartialEq + Copy + Into<f64> + std::fmt::Debug> CountedVec<T> {
    pub fn weighted_average(&self, threshold: f64) -> f64 {
        let mut total_count = 0;

        // gather total count first
        for value in self.data.iter() {
            total_count += value.count;
        }

        let mut total_value = 0.0;
        let mut total_threshold_count = 0;

        for value in self.data.iter() {
            // only consider if above threshold
            if value.count as f64 > total_count as f64 * threshold {
                total_value += value.count as f64 * value.value.into();
                total_threshold_count += value.count;
            }
        }

        println!(
            "total_value ({:.2}) / total_threshold_count ({:.2}) = {:.2}",
            total_value,
            total_threshold_count,
            total_value / total_threshold_count as f64
        );

        for date in &self.data {
            println!("count: {}, value: {:.2}", date.count, date.value.into());
        }

        if total_threshold_count == 0 {
            0.0
        } else {
            total_value / total_threshold_count as f64
        }
    }
}
