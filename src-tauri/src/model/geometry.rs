use super::*;

use gdsii::I32;
use gdsii::parser::{Path as GdsPath, Strans};
use gdsii::types::GdsPoint;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform2d {
    xx: f32,
    xy: f32,
    yx: f32,
    yy: f32,
    tx: f32,
    ty: f32,
}

impl Transform2d {
    pub(super) fn identity() -> Self {
        Self {
            xx: 1.0,
            xy: 0.0,
            yx: 0.0,
            yy: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub(super) fn from_strans(strans: Option<Strans>) -> Self {
        let Some(strans) = strans else {
            return Self::identity();
        };
        let magnification = strans.mag.map(f64::from).unwrap_or(1.0) as f32;
        let angle = (strans.angle.map(f64::from).unwrap_or(0.0) as f32).to_radians();
        let sin = angle.sin() * magnification;
        let cos = angle.cos() * magnification;
        let reflect = if strans.reflection { -1.0 } else { 1.0 };

        Self {
            xx: cos,
            xy: -sin * reflect,
            yx: sin,
            yy: cos * reflect,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub(super) fn with_translation(mut self, origin: [f32; 2]) -> Self {
        self.tx = origin[0];
        self.ty = origin[1];
        self
    }

    pub(super) fn with_offset(mut self, offset: [f32; 2]) -> Self {
        self.tx += offset[0];
        self.ty += offset[1];
        self
    }

    pub(super) fn then(self, next: Self) -> Self {
        Self {
            xx: self.xx * next.xx + self.xy * next.yx,
            xy: self.xx * next.xy + self.xy * next.yy,
            yx: self.yx * next.xx + self.yy * next.yx,
            yy: self.yx * next.xy + self.yy * next.yy,
            tx: self.xx * next.tx + self.xy * next.ty + self.tx,
            ty: self.yx * next.tx + self.yy * next.ty + self.ty,
        }
    }

    fn apply(self, point: [f32; 2]) -> [f32; 2] {
        [
            self.xx * point[0] + self.xy * point[1] + self.tx,
            self.yx * point[0] + self.yy * point[1] + self.ty,
        ]
    }
}

pub(super) fn parse_coordinate_scale(db_in_user: f64) -> anyhow::Result<f32> {
    if !db_in_user.is_finite() || db_in_user <= 0.0 {
        anyhow::bail!("invalid GDS library unit scale: {db_in_user}");
    }
    if db_in_user > f64::from(f32::MAX) {
        anyhow::bail!("GDS library unit scale is too large: {db_in_user}");
    }
    Ok(db_in_user as f32)
}

pub(super) fn point_from_xy(xy: &[I32], index: usize, coordinate_scale: f32) -> Option<[f32; 2]> {
    let point = GdsPoint::iter_xy(xy).nth(index)?;
    let x = point.x as f32 * coordinate_scale;
    let y = point.y as f32 * coordinate_scale;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some([x, y])
}

pub(super) fn step_vector(origin: [f32; 2], end: [f32; 2], count: usize) -> [f32; 2] {
    let divisor = count as f32;
    [
        (end[0] - origin[0]) / divisor,
        (end[1] - origin[1]) / divisor,
    ]
}

pub(super) fn path_shape_from_gds(path: &GdsPath<'_>, coordinate_scale: f32) -> Option<PathShape> {
    let width = path.width?.unsigned_abs() as f32 * coordinate_scale;
    if !width.is_finite() || width <= 0.0 {
        return None;
    }

    let points = GdsPoint::iter_xy(path.xy.as_ref())
        .map(|point| {
            [
                point.x as f32 * coordinate_scale,
                point.y as f32 * coordinate_scale,
            ]
        })
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return None;
    }
    Some(PathShape {
        centerline: points,
        width,
        cap: path_cap_style_from_gds(path, coordinate_scale),
    })
}

fn path_cap_style_from_gds(path: &GdsPath<'_>, coordinate_scale: f32) -> PathCapStyle {
    match path.pathtype.unwrap_or_default() {
        1 => PathCapStyle::Round,
        2 => PathCapStyle::ExtendedHalfWidth,
        4 => PathCapStyle::Custom {
            begin_extension: path.bgn_extn.unwrap_or_default() as f32 * coordinate_scale,
            end_extension: path.end_extn.unwrap_or_default() as f32 * coordinate_scale,
        },
        _ => PathCapStyle::Flush,
    }
}

pub(super) fn shape_kind_from_box(polygon: Polygon2d) -> ShapeKind {
    rectangle_from_polygon(&polygon)
        .map(ShapeKind::Rectangle)
        .unwrap_or(ShapeKind::Boundary(polygon))
}

fn rectangle_from_polygon(polygon: &Polygon2d) -> Option<RectangleShape> {
    if polygon.points.len() != 4 || !polygon.holes.is_empty() {
        return None;
    }
    let bounds = polygon_bounds(polygon)?;
    if polygon.points.iter().any(|[x, y]| {
        (*x != bounds.min_x && *x != bounds.max_x) || (*y != bounds.min_y && *y != bounds.max_y)
    }) {
        return None;
    }
    Some(RectangleShape {
        center: [
            (bounds.min_x + bounds.max_x) * 0.5,
            (bounds.min_y + bounds.max_y) * 0.5,
        ],
        size: [bounds.max_x - bounds.min_x, bounds.max_y - bounds.min_y],
        rotation: 0.0,
    })
}

pub(super) fn shape_polygons(shape: &ShapeKind) -> Vec<Polygon2d> {
    match shape {
        ShapeKind::Boundary(polygon) => vec![polygon.clone()],
        ShapeKind::Path(path) => path_polygon_from_points(&path.centerline, path.width)
            .into_iter()
            .collect(),
        ShapeKind::Rectangle(rectangle) => rectangle_polygon(rectangle).into_iter().collect(),
    }
}

fn rectangle_polygon(rectangle: &RectangleShape) -> Option<Polygon2d> {
    let half_width = rectangle.size[0] * 0.5;
    let half_height = rectangle.size[1] * 0.5;
    if !half_width.is_finite()
        || !half_height.is_finite()
        || half_width <= 0.0
        || half_height <= 0.0
        || !rectangle.rotation.is_finite()
    {
        return None;
    }
    let (sin, cos) = rectangle.rotation.to_radians().sin_cos();
    let points = [
        [-half_width, -half_height],
        [half_width, -half_height],
        [half_width, half_height],
        [-half_width, half_height],
    ]
    .into_iter()
    .map(|[x, y]| {
        [
            rectangle.center[0] + x * cos - y * sin,
            rectangle.center[1] + x * sin + y * cos,
        ]
    })
    .collect::<Vec<_>>();
    let polygon = Polygon2d {
        points,
        holes: Vec::new(),
    };
    polygon_bounds(&polygon)?;
    Some(polygon)
}

fn path_polygon_from_points(points: &[[f32; 2]], width: f32) -> Option<Polygon2d> {
    if points.len() < 2 {
        return None;
    }

    let half_width = width * 0.5;
    let mut normals = Vec::with_capacity(points.len().saturating_sub(1));
    for segment in points.windows(2) {
        normals.push(segment_normal(segment[0], segment[1])?);
    }

    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let normal = if index == 0 {
            normals[0]
        } else if index == points.len() - 1 {
            normals[normals.len() - 1]
        } else {
            average_normal(normals[index - 1], normals[index])
        };
        left.push([
            points[index][0] + normal[0] * half_width,
            points[index][1] + normal[1] * half_width,
        ]);
        right.push([
            points[index][0] - normal[0] * half_width,
            points[index][1] - normal[1] * half_width,
        ]);
    }

    right.reverse();
    let mut polygon = Polygon2d {
        points: left,
        holes: Vec::new(),
    };
    polygon.points.extend(right);
    polygon_bounds(&polygon)?;
    Some(polygon)
}

fn segment_normal(start: [f32; 2], end: [f32; 2]) -> Option<[f32; 2]> {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f32::EPSILON {
        return None;
    }
    Some([-dy / length, dx / length])
}

fn average_normal(previous: [f32; 2], next: [f32; 2]) -> [f32; 2] {
    let x = previous[0] + next[0];
    let y = previous[1] + next[1];
    let length = x.hypot(y);
    if !length.is_finite() || length <= f32::EPSILON {
        return next;
    }
    [x / length, y / length]
}

pub(super) fn transform_polygon(polygon: &Polygon2d, transform: Transform2d) -> Option<Polygon2d> {
    let mut transformed = Polygon2d {
        points: Vec::with_capacity(polygon.points.len()),
        holes: polygon
            .holes
            .iter()
            .map(|hole| Vec::with_capacity(hole.len()))
            .collect(),
    };
    for point in &polygon.points {
        let next = transform.apply(*point);
        if !next[0].is_finite() || !next[1].is_finite() {
            return None;
        }
        if transformed.points.last().is_some_and(|last| *last == next) {
            continue;
        }
        transformed.points.push(next);
    }

    for (hole, transformed_hole) in polygon.holes.iter().zip(&mut transformed.holes) {
        for point in hole {
            let next = transform.apply(*point);
            if !next[0].is_finite() || !next[1].is_finite() {
                return None;
            }
            transformed_hole.push(next);
        }
    }

    if transformed.points.len() >= 2 && transformed.points.first() == transformed.points.last() {
        transformed.points.pop();
    }
    if transformed.points.len() < 3 {
        return None;
    }
    polygon_bounds(&transformed)?;
    Some(transformed)
}

pub(super) fn polygon_from_points(
    points: impl IntoIterator<Item = GdsPoint>,
    coordinate_scale: f32,
) -> Option<Polygon2d> {
    let mut polygon = Polygon2d {
        points: Vec::new(),
        holes: Vec::new(),
    };
    for point in points {
        let x = point.x as f32 * coordinate_scale;
        let y = point.y as f32 * coordinate_scale;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let next = [x, y];
        if polygon.points.last().is_some_and(|last| *last == next) {
            continue;
        }
        polygon.points.push(next);
    }

    if polygon.points.len() >= 2 && polygon.points.first() == polygon.points.last() {
        polygon.points.pop();
    }
    if polygon.points.len() < 3 {
        return None;
    }
    polygon_bounds(&polygon)?;
    Some(polygon)
}

pub(super) fn polygon_bounds(polygon: &Polygon2d) -> Option<Bounds2d> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for [x, y] in &polygon.points {
        min_x = min_x.min(*x);
        min_y = min_y.min(*y);
        max_x = max_x.max(*x);
        max_y = max_y.max(*y);
    }

    if min_x >= max_x || min_y >= max_y {
        return None;
    }
    Some(Bounds2d {
        min_x,
        min_y,
        max_x,
        max_y,
    })
}

pub(super) fn polygons_bounds(polygons: &[Polygon2d]) -> Option<Bounds2d> {
    let mut bounds = None;
    for polygon in polygons {
        merge_optional_bounds(&mut bounds, &polygon_bounds(polygon)?);
    }
    bounds
}

pub(super) fn union_layer_polygons(polygons: Vec<Polygon2d>) -> Vec<Polygon2d> {
    if polygons.len() < 2 {
        return polygons;
    }

    let mut contours = Vec::new();
    for polygon in polygons {
        let mut outer = polygon.points;
        orient_contour(&mut outer, true);
        contours.push(outer);
        for mut hole in polygon.holes {
            orient_contour(&mut hole, false);
            contours.push(hole);
        }
    }

    contours
        .simplify_shape(FillRule::NonZero)
        .into_iter()
        .filter_map(|mut shape| {
            if shape.is_empty() {
                return None;
            }
            let points = shape.remove(0);
            let polygon = Polygon2d {
                points,
                holes: shape,
            };
            polygon_bounds(&polygon)?;
            Some(polygon)
        })
        .collect()
}

fn orient_contour(contour: &mut [[f32; 2]], counterclockwise: bool) {
    let area = signed_contour_area(contour);
    if (area > 0.0) != counterclockwise {
        contour.reverse();
    }
}

pub(super) fn signed_contour_area(contour: &[[f32; 2]]) -> f64 {
    if contour.len() < 3 {
        return 0.0;
    }
    contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .take(contour.len())
        .map(|([x1, y1], [x2, y2])| {
            f64::from(*x1) * f64::from(*y2) - f64::from(*x2) * f64::from(*y1)
        })
        .sum::<f64>()
        * 0.5
}

pub(super) fn merge_bounds(target: &mut Bounds2d, other: &Bounds2d) {
    target.min_x = target.min_x.min(other.min_x);
    target.min_y = target.min_y.min(other.min_y);
    target.max_x = target.max_x.max(other.max_x);
    target.max_y = target.max_y.max(other.max_y);
}

pub(super) fn merge_optional_bounds(target: &mut Option<Bounds2d>, other: &Bounds2d) {
    match target {
        Some(bounds) => merge_bounds(bounds, other),
        None => *target = Some(other.clone()),
    }
}

pub(super) fn is_metadata_cell(name: &str) -> bool {
    name.starts_with("$$$") && name.ends_with("$$$")
}
