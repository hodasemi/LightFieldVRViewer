#[derive(Debug, Clone)]
pub struct CountedRange {
    allowed_distance_to_average: f32,

    max: f32,
    min: f32,

    values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct Ranges {
    allowed_distance_to_average: f32,

    total_element_count: usize,

    data: Vec<CountedRange>,
}

impl Ranges {
    pub fn new(allowed_distance_to_average: f32) -> Ranges {
        Ranges {
            allowed_distance_to_average,

            total_element_count: 0,

            data: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    // pub fn len(&self) -> usize {
    //     self.data.len()
    // }

    // pub fn element_count(&self) -> usize {
    //     self.total_element_count
    // }

    pub fn weighted_average(&self, threshold: f64) -> Option<f64> {
        let mut weight_count = 0;
        let mut averages = 0.0;

        for range in self.data.iter() {
            if range.len() as f64 >= (self.total_element_count as f64 * threshold) {
                weight_count += range.len();
                averages += range.sum();
            }
        }

        if weight_count != 0 {
            Some(averages as f64 / weight_count as f64)
        } else {
            None
        }
    }

    pub fn insert(&mut self, value: f32) {
        self.total_element_count += 1;

        for range in self.data.iter_mut() {
            if range.insert(value) {
                return;
            }
        }

        self.add_new_range(value);
    }

    fn add_new_range(&mut self, value: f32) {
        let mut new_range = CountedRange::new(self.allowed_distance_to_average);

        new_range.insert(value);

        self.data.push(new_range);
    }
}

impl CountedRange {
    pub fn new(allowed_distance_to_average: f32) -> CountedRange {
        CountedRange {
            allowed_distance_to_average,

            max: 0.0,
            min: 0.0,

            values: Vec::new(),
        }
    }

    fn sum(&self) -> f32 {
        self.values.iter().sum()
    }

    pub fn average(&self) -> f32 {
        self.sum() / self.values.len() as f32
    }

    fn insert(&mut self, value: f32) -> bool {
        if self.values.is_empty() {
            self.max = value;
            self.min = value;
        } else {
            let current_average = self.average();

            if value > (current_average + self.allowed_distance_to_average)
                || value < (current_average - self.allowed_distance_to_average)
            {
                return false;
            }

            self.max = self.max.max(value);
            self.min = self.min.min(value);
        }

        self.values.push(value);

        true
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}
