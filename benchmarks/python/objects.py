class Shape:
    def __init__(self, scale):
        self.scale = scale

    def area(self):
        return self.scale


class Rectangle(Shape):
    def __init__(self, width, height):
        super().__init__(1.0)
        self.width = width
        self.height = height

    def area(self):
        return self.width * self.height * self.scale


class Square(Rectangle):
    def __init__(self, side):
        super().__init__(side, side)

    def area(self):
        return self.width * self.width * self.scale


def area_of(shape):
    return shape.area()


def make(i):
    if i % 3 == 0:
        return Shape(1.0)
    if i % 3 == 1:
        return Rectangle(2.0, 3.0)
    return Square(4.0)


shapes = [make(i) for i in range(2_000_000)]
total = 0.0
for shape in shapes:
    total += area_of(shape)
print(f"{total:.1f}")
