"""The two lines of `mathutils` the converter script uses.

`report_bounds` carries each corner of each object's bounding box through
that object's world matrix, so a vector needs three components and a
matrix needs to multiply one. The fake `bpy` beside this hands out
identity matrices, which is all a scene nobody moved anything in has.
"""


class Vector:
    def __init__(self, values):
        self.x, self.y, self.z = (float(value) for value in values)

    def __iter__(self):
        return iter((self.x, self.y, self.z))
