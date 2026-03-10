use std::f64::consts::PI;

pub enum Shape
 {
    Rectangle { width: f64, height: f64 },
    Circle { radius: f64 },
    Triangle { base: f64, height: f64 },
}

impl Shape {
    pub fn area(&self) -> f64 {
        match self {
            Shape::Rectangle { width, height } => width * height,
            Shape::Circle { radius } => PI * radius * radius,
            Shape::Triangle { base, height } => 0.5 * base * height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_rectangle_area() {
        let rect = Shape::Rectangle { width: 5.0, height: 10.0 };
        assert_eq!(rect.area(), 50.0);
    }   
    #[test]
    fn test_circle_area() {
        let circle = Shape::Circle { radius: 3.0 };
        assert_eq!(circle.area(), PI * 9.0);
    }
    #[test]
    fn test_triangle_area() {
        let triangle = Shape::Triangle { base: 4.0, height: 5.0 };
        assert_eq!(triangle.area(), 10.0);
    }
}