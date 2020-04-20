use super::na;
use super::na::{Transform2, Point2, Matrix3};

pub type Transform = Transform2<f32>;
pub type Point = Point2<f32>;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    bottom_left: Point,
    top_right: Point
}

pub fn transform_from_args(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Transform {
    let matrix = Matrix3::new(a, b, c, d, e, f, 0.0, 0.0, 1.0);
    na::try_convert(matrix).unwrap()
}

pub fn transform_from_vec(mut args: Vec<f32>) -> Transform {
    if args.len() != 6 {
        // Panic in debug, otherwise zero-extend
        debug_assert_eq!(1, 0, "Function transform_from_vec requires 6 arguments!");
        let padding = 6 - args.len();
        for _ in 0..padding {
            args.push(0.0);
        }
     }
    let matrix = Matrix3::new(args[0], args[1], args[2], args[3], args[4], args[5], 0.0, 0.0, 1.0);
    //println!("Matrix: {:?}", matrix);
    na::try_convert(matrix).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn identity_matrix() {
        let id = transform_from_args(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let points = vec!(
            Point::new(2.0, 4.0),
            Point::new(-1.0, 4.0),
            Point::new(-1.3232, -4.32312),
            Point::new(1085345.0, -4.32312),
            Point::new(0.0, 0.0),
            Point::new(-73.2, 0.0),
            Point::new(0.0, 324.4)
        );
        for p in points {
            assert_eq!(p, id * p);
        }
    }

    #[test]
    fn create_new_matrix() {
        let my_matrix = transform_from_args(-1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    }
    
}