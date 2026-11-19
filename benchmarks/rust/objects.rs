trait Shape {
    fn area(&self) -> f64;
}

struct Base {
    scale: f64,
}

impl Shape for Base {
    fn area(&self) -> f64 {
        self.scale
    }
}

struct Rectangle {
    width: f64,
    height: f64,
    scale: f64,
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height * self.scale
    }
}

struct Square {
    width: f64,
    scale: f64,
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.width * self.width * self.scale
    }
}

fn area_of(shape: &dyn Shape) -> f64 {
    shape.area()
}

fn main() {
    let mut shapes: Vec<Box<dyn Shape>> = Vec::with_capacity(2_000_000);
    for i in 0..2_000_000i64 {
        let shape: Box<dyn Shape> = match i % 3 {
            0 => Box::new(Base { scale: 1.0 }),
            1 => Box::new(Rectangle {
                width: 2.0,
                height: 3.0,
                scale: 1.0,
            }),
            _ => Box::new(Square {
                width: 4.0,
                scale: 1.0,
            }),
        };
        shapes.push(shape);
    }
    let mut total = 0.0;
    for shape in &shapes {
        total += area_of(&**shape);
    }
    println!("{:.1}", total);
}
