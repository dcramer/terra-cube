//! A model's skeleton and what its bones are for. Tripo names bones unreliably (`bone_7` beside
//! `tripo::0_Right_Limb_0`), so roles are read from the skeleton's shape, never from names:
//! chains that reach the ground are legs, the highest other chain is the head, one trailing
//! behind is the tail. Clips are then written against roles (clips.rs), as Spore did.

use crate::glb::Glb;
use anyhow::{Context, Result, ensure};
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;

pub struct Joint {
    pub node: usize,
    pub name: String,
    /// Index into `Skeleton::joints`; None for the root joint.
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub rest_t: Vec3,
    pub rest_r: Quat,
    pub rest_s: Vec3,
    /// Rest transform in the file's space.
    pub world: Mat4,
}

pub struct Skeleton {
    pub joints: Vec<Joint>,
    pub root: usize,
    /// The world transform of the root joint's parent node, which never animates.
    pub base: Mat4,
    pub inverse_bind: Vec<Mat4>,
}

/// A pose: each joint's local rotation, and the root joint's local translation.
#[derive(Clone)]
pub struct Pose {
    pub rotations: Vec<Quat>,
    pub root_t: Vec3,
}

fn node_local(n: &Value) -> (Vec3, Quat, Vec3) {
    if let Some(m) = n.get("matrix").and_then(Value::as_array) {
        let cols: Vec<f32> = m.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect();
        let (s, r, t) = Mat4::from_cols_slice(&cols).to_scale_rotation_translation();
        return (t, r, s);
    }
    let f = |key: &str, default: &[f32]| -> Vec<f32> {
        n.get(key).and_then(Value::as_array).map_or(default.to_vec(), |a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
    };
    let (t, r, s) = (f("translation", &[0.0; 3]), f("rotation", &[0.0, 0.0, 0.0, 1.0]), f("scale", &[1.0; 3]));
    (Vec3::from_slice(&t), Quat::from_slice(&r).normalize(), Vec3::from_slice(&s))
}

impl Skeleton {
    pub fn read(glb: &Glb) -> Result<Self> {
        let nodes = glb.array("nodes");
        let skin = glb.array("skins").first().context("the model has no skin: it is not rigged")?;
        let joint_nodes: Vec<usize> = skin["joints"].as_array().context("skin without joints")?.iter().map(|v| v.as_u64().unwrap() as usize).collect();
        let mut node_parent = vec![None; nodes.len()];
        for (i, n) in nodes.iter().enumerate() {
            for c in n["children"].as_array().into_iter().flatten() {
                node_parent[c.as_u64().unwrap() as usize] = Some(i);
            }
        }
        fn world(i: usize, nodes: &[Value], parent: &[Option<usize>]) -> Mat4 {
            let (t, r, s) = node_local(&nodes[i]);
            let local = Mat4::from_scale_rotation_translation(s, r, t);
            parent[i].map_or(local, |p| world(p, nodes, parent) * local)
        }
        let index_of = |node: usize| joint_nodes.iter().position(|&j| j == node);
        let mut joints: Vec<Joint> = joint_nodes
            .iter()
            .map(|&node| {
                let (rest_t, rest_r, rest_s) = node_local(&nodes[node]);
                Joint {
                    node,
                    name: nodes[node]["name"].as_str().unwrap_or("").to_owned(),
                    parent: node_parent[node].and_then(index_of),
                    children: Vec::new(),
                    rest_t,
                    rest_r,
                    rest_s,
                    world: world(node, nodes, &node_parent),
                }
            })
            .collect();
        for i in 0..joints.len() {
            if let Some(p) = joints[i].parent {
                joints[p].children.push(i);
            }
        }
        let roots: Vec<usize> = (0..joints.len()).filter(|&i| joints[i].parent.is_none()).collect();
        ensure!(roots.len() == 1, "expected one root joint, found {}", roots.len());
        let root = roots[0];
        let base = node_parent[joints[root].node].map_or(Mat4::IDENTITY, |p| world(p, nodes, &node_parent));
        let inverse_bind = match skin.get("inverseBindMatrices").and_then(Value::as_u64) {
            Some(a) => glb.floats(a as usize)?.0.chunks(16).map(Mat4::from_cols_slice).collect(),
            None => vec![Mat4::IDENTITY; joints.len()],
        };
        Ok(Self { joints, root, base, inverse_bind })
    }

    pub fn rest(&self) -> Pose {
        Pose { rotations: self.joints.iter().map(|j| j.rest_r).collect(), root_t: self.joints[self.root].rest_t }
    }

    pub fn position(&self, joint: usize) -> Vec3 {
        self.joints[joint].world.w_axis.truncate()
    }

    /// World matrices of every joint in a pose.
    pub fn pose_world(&self, pose: &Pose) -> Vec<Mat4> {
        let mut out = vec![Mat4::IDENTITY; self.joints.len()];
        let mut done = vec![false; self.joints.len()];
        fn go(i: usize, sk: &Skeleton, pose: &Pose, out: &mut [Mat4], done: &mut [bool]) {
            if done[i] {
                return;
            }
            let j = &sk.joints[i];
            let (parent, t) = match j.parent {
                Some(p) => {
                    go(p, sk, pose, out, done);
                    (out[p], j.rest_t)
                }
                None => (sk.base, pose.root_t),
            };
            out[i] = parent * Mat4::from_scale_rotation_translation(j.rest_s, pose.rotations[i], t);
            done[i] = true;
        }
        for i in 0..self.joints.len() {
            go(i, self, pose, &mut out, &mut done);
        }
        out
    }

    /// The local rotation that turns a joint by `delta`, a rotation in the file's space, from
    /// its rest pose. Chained down a limb it curls, since each bone's axis rides its parent.
    pub fn turned(&self, joint: usize, delta: Quat) -> Quat {
        let (_, world_r, _) = self.joints[joint].world.to_scale_rotation_translation();
        (self.joints[joint].rest_r * (world_r.inverse() * delta * world_r)).normalize()
    }

    /// The root's local translation moved by `offset` in the file's space.
    pub fn root_moved(&self, offset: Vec3) -> Vec3 {
        self.joints[self.root].rest_t + self.base.inverse().transform_vector3(offset)
    }
}

pub struct Leg {
    /// From the hip down to the foot.
    pub chain: Vec<usize>,
    pub front: bool,
    pub left: bool,
}

impl Leg {
    pub fn foot(&self) -> usize {
        *self.chain.last().unwrap()
    }
}

pub struct Roles {
    pub forward: Vec3,
    /// The creature's left.
    pub side: Vec3,
    pub legs: Vec<Leg>,
    /// From the neck out to the tip of the head; empty when nothing reads as a head.
    pub head: Vec<usize>,
    pub tail: Vec<usize>,
    /// The trunk between the root and where the head branches off.
    pub spine: Vec<usize>,
    /// Every other chain: arms, wings, ears, fins.
    pub others: Vec<Vec<usize>>,
    /// Limbs that leave the trunk and end in a branching hand: shoulder, upper arm, forearm,
    /// hand. Their fingers are in `others`.
    pub arms: Vec<Leg>,
    /// Skeleton extent: lowest and highest joint, and length along `forward`.
    pub floor: f32,
    pub height: f32,
    pub length: f32,
}

/// Which way a model faces: read from the skeleton, or told.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Facing {
    Auto,
    Told(Vec3),
}

impl Facing {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "auto" => Facing::Auto,
            "+x" => Facing::Told(Vec3::X),
            "-x" => Facing::Told(Vec3::NEG_X),
            "+z" => Facing::Told(Vec3::Z),
            "-z" => Facing::Told(Vec3::NEG_Z),
            other => anyhow::bail!("facing must be auto, +x, -x, +z or -z, not {other}"),
        })
    }
}

impl Roles {
    pub fn read(sk: &Skeleton, facing: Facing) -> Self {
        // Chains: from each leaf up to, not including, the first bone that branches.
        let mut chains: Vec<Vec<usize>> = Vec::new();
        for leaf in (0..sk.joints.len()).filter(|&i| sk.joints[i].children.is_empty() && i != sk.root) {
            let mut chain = vec![leaf];
            let mut at = leaf;
            while let Some(p) = sk.joints[at].parent {
                if p == sk.root || sk.joints[p].children.len() != 1 {
                    break;
                }
                chain.push(p);
                at = p;
            }
            chain.reverse();
            chains.push(chain);
        }
        let ys = sk.joints.iter().map(|j| j.world.w_axis.y);
        let (floor, top) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        let height = (top - floor).max(1e-4);
        let tip = |c: &Vec<usize>| sk.position(*c.last().unwrap());
        let (legs, rest): (Vec<_>, Vec<_>) = chains.into_iter().partition(|c| c.len() >= 2 && tip(c).y - floor < 0.2 * height);
        let feet = legs.iter().map(&tip).sum::<Vec3>() / legs.len().max(1) as f32;
        let centre = if legs.is_empty() { sk.position(sk.root) } else { feet };

        let head_at = rest.iter().enumerate().max_by(|a, b| tip(a.1).y.total_cmp(&tip(b.1).y)).map(|(i, _)| i);
        let forward = match facing {
            Facing::Told(v) => v,
            Facing::Auto => {
                // Toward the head when it sits clearly ahead of the feet (an animal); else the
                // way Tripo exports an upright figure, +Z.
                let lean = head_at.map_or(Vec3::ZERO, |i| (tip(&rest[i]) - centre) * Vec3::new(1.0, 0.0, 1.0));
                if lean.length() < 0.15 * height {
                    Vec3::Z
                } else if lean.x.abs() > lean.z.abs() {
                    Vec3::X * lean.x.signum()
                } else {
                    Vec3::Z * lean.z.signum()
                }
            }
        };
        let side = Vec3::Y.cross(forward);
        let along = |p: Vec3| (p - centre).dot(forward);
        let (lo, hi) = sk.joints.iter().map(|j| along(j.world.w_axis.truncate())).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let length = (hi - lo).max(1e-4);

        let mut head = Vec::new();
        let mut tail = Vec::new();
        let mut others = Vec::new();
        let tail_at = rest
            .iter()
            .enumerate()
            .filter(|(i, c)| Some(*i) != head_at && along(tip(c)) < -0.25 * length && (tip(c) - centre).dot(side).abs() < 0.15 * length)
            .min_by(|a, b| along(tip(a.1)).total_cmp(&along(tip(b.1))))
            .map(|(i, _)| i);
        for (i, chain) in rest.into_iter().enumerate() {
            if Some(i) == head_at {
                head = chain;
            } else if Some(i) == tail_at {
                tail = chain;
            } else {
                others.push(chain);
            }
        }
        // The trunk: from the root to where the head chain starts, or the longest branching path.
        let mut spine = Vec::new();
        let mut at = head.first().and_then(|&h| sk.joints[h].parent);
        while let Some(j) = at {
            if j == sk.root {
                break;
            }
            spine.push(j);
            at = sk.joints[j].parent;
        }
        spine.reverse();
        let legs = legs
            .into_iter()
            .map(|chain| {
                let p = tip(&chain);
                Leg { front: along(p) > 0.0, left: (p - centre).dot(side) > 0.0, chain }
            })
            .collect();
        let mut arms: Vec<Leg> = Vec::new();
        for chain in &others {
            let mut path = Vec::new();
            let mut at = sk.joints[chain[0]].parent;
            while let Some(j) = at.filter(|&j| j != sk.root && !spine.contains(&j)) {
                path.push(j);
                at = sk.joints[j].parent;
            }
            path.reverse();
            if path.len() >= 2 && !arms.iter().any(|a| a.chain[0] == path[0]) {
                let left = (sk.position(*path.last().unwrap()) - centre).dot(side) > 0.0;
                arms.push(Leg { front: true, left, chain: path });
            }
        }
        Roles { forward, side, legs, head, tail, spine, others, arms, floor, height, length }
    }

    /// One line a role, for `forge inspect`.
    pub fn describe(&self, sk: &Skeleton) -> String {
        let names = |c: &[usize]| c.iter().map(|&j| sk.joints[j].name.as_str()).collect::<Vec<_>>().join(" > ");
        let mut out = format!("forward {:?}  height {:.2}  length {:.2}  joints {}\n", self.forward, self.height, self.length, sk.joints.len());
        out += &format!("spine  {}\nhead   {}\ntail   {}\n", names(&self.spine), names(&self.head), names(&self.tail));
        for l in &self.legs {
            out += &format!("leg {}{} {}\n", if l.front { "front" } else { "hind " }, if l.left { " left " } else { " right" }, names(&l.chain));
        }
        for a in &self.arms {
            out += &format!("arm {} {}\n", if a.left { "left " } else { "right" }, names(&a.chain));
        }
        for o in &self.others {
            out += &format!("other  {}\n", names(o));
        }
        out
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A four-legged stick animal facing +X whose bones all have awkward rest frames, as
    /// Tripo's do: root, two spine bones, a two-bone head, a two-bone tail, four two-bone legs.
    pub fn animal() -> Skeleton {
        let twist = Quat::from_euler(glam::EulerRot::XYZ, 0.7, -1.1, 2.0);
        let mut joints: Vec<Joint> = Vec::new();
        let add = |name: &str, parent: Option<usize>, at: Vec3, joints: &mut Vec<Joint>| -> usize {
            let world = Mat4::from_rotation_translation(twist, at);
            let local = parent.map_or(world, |p: usize| joints[p].world.inverse() * world);
            let (rest_s, rest_r, rest_t) = local.to_scale_rotation_translation();
            joints.push(Joint { node: joints.len(), name: name.into(), parent, children: Vec::new(), rest_t, rest_r, rest_s, world });
            let i = joints.len() - 1;
            if let Some(p) = parent {
                joints[p].children.push(i);
            }
            i
        };
        let root = add("root", None, Vec3::new(0.0, 0.5, 0.0), &mut joints);
        let hips = add("a", Some(root), Vec3::new(-0.3, 0.5, 0.0), &mut joints);
        let chest = add("b", Some(hips), Vec3::new(0.3, 0.5, 0.0), &mut joints);
        let neck = add("c", Some(chest), Vec3::new(0.4, 0.6, 0.0), &mut joints);
        add("d", Some(neck), Vec3::new(0.6, 0.8, 0.0), &mut joints);
        let tail = add("e", Some(root), Vec3::new(-0.4, 0.5, 0.0), &mut joints);
        add("f", Some(tail), Vec3::new(-0.7, 0.4, 0.0), &mut joints);
        for (parent, x) in [(chest, 0.3), (hips, -0.3)] {
            for z in [0.1, -0.1] {
                let hip = add("g", Some(parent), Vec3::new(x, 0.45, z), &mut joints);
                let knee = add("h", Some(hip), Vec3::new(x, 0.2, z), &mut joints);
                add("i", Some(knee), Vec3::new(x, 0.0, z), &mut joints);
            }
        }
        let n = joints.len();
        let inverse_bind = joints.iter().map(|j| j.world.inverse()).collect();
        let _ = n;
        Skeleton { joints, root, base: Mat4::IDENTITY, inverse_bind }
    }

    #[test]
    fn roles_are_read_from_shape() {
        let sk = animal();
        let r = Roles::read(&sk, Facing::Auto);
        assert_eq!(r.forward, Vec3::X);
        assert_eq!((r.legs.len(), r.head.len(), r.tail.len(), r.spine.len()), (4, 2, 2, 2));
        assert_eq!(r.legs.iter().filter(|l| l.front).count(), 2);
        // +Z is the left of something facing +X? No: left is up × forward = -Z.
        assert!(r.legs.iter().filter(|l| l.left).all(|l| sk.position(l.foot()).z < 0.0));
    }

    #[test]
    fn rest_pose_reproduces_rest_world() {
        let sk = animal();
        for (w, j) in sk.pose_world(&sk.rest()).iter().zip(&sk.joints) {
            assert!(w.abs_diff_eq(j.world, 1e-4));
        }
    }

    #[test]
    fn a_turn_is_in_the_files_space_whatever_the_bone_frame() {
        let sk = animal();
        let r = Roles::read(&sk, Facing::Auto);
        let (neck, tip) = (r.head[0], r.head[1]);
        let mut pose = sk.rest();
        // Positive pitch tips the nose down.
        pose.rotations[neck] = sk.turned(neck, Quat::from_axis_angle(r.side, 0.5));
        let w = sk.pose_world(&pose);
        assert!(w[tip].w_axis.y < sk.position(tip).y - 0.05, "{} vs {}", w[tip].w_axis.y, sk.position(tip).y);
        assert!(w[tip].w_axis.x > sk.position(neck).x);
    }
}
