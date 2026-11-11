"""Puts captured human motion on a creature's skeleton, in Blender, and writes it out for forge.

    python retarget.py RIGGED.glb OUT.glb NAME=TAKE[:lo-hi] ...

RIGGED.glb is Tripo's export of a biped (its bones have Mixamo's names); each NAME=TAKE is a
clip to make (`walk`, `run`) from a take of the CMU motion capture library: its number
(`07_02`, fetched once into .work/cmu from a mirror of the library's BVH files) or a file.
The library is free for any use: "This data is free for use in research and commercial
projects worldwide" (mocap.cs.cmu.edu), made with funding from NSF EIA-0196217. `lo-hi`
is how long one cycle of it may be, seconds (the best-matching pair of frames that far apart
is found, and the clip is what lies between); without it the whole take is used. OUT.glb is
the same model with those clips added under those names; a clip of the same name that was
there is kept as `tripo_NAME`. `forge finish` reads it in place of the export.

How: a BVH's rest pose is every joint unturned, which for these takes is a T-pose; the
creature's rest pose is a T-pose too. So a bone's turn from rest in the take, carried from
the take's axes (y up, facing +z) to Blender's (z up, facing -y), is its turn from rest on
the creature. The hips' travel over the ground is taken out (the game moves the body; a clip
plays in place) and their rise and fall and sway are kept, to the creature's size.

Run with the Python that has Blender in it (`make blender` makes it); `cargo forge motion ID` runs this.
"""
import math, os, re, sys, urllib.request

import bpy
from mathutils import Matrix, Quaternion, Vector

# The take's bone for each of the creature's.
BONES = {
    "Hips": "Hips", "Spine": "LowerBack", "Spine1": "Spine", "Spine2": "Spine1", "Neck": "Neck", "Head": "Head",
    "LeftShoulder": "LeftShoulder", "LeftArm": "LeftArm", "LeftForeArm": "LeftForeArm", "LeftHand": "LeftHand",
    "RightShoulder": "RightShoulder", "RightArm": "RightArm", "RightForeArm": "RightForeArm", "RightHand": "RightHand",
    "LeftUpLeg": "LeftUpLeg", "LeftLeg": "LeftLeg", "LeftFoot": "LeftFoot", "LeftToeBase": "LeftToeBase",
    "RightUpLeg": "RightUpLeg", "RightLeg": "RightLeg", "RightFoot": "RightFoot", "RightToeBase": "RightToeBase",
}
# The take's axes to Blender's: y up to z up, facing +z to facing -y.
AXES = Matrix(((1, 0, 0), (0, 0, -1), (0, 1, 0)))
FPS = 30


MIRROR = "https://raw.githubusercontent.com/una-dinosauria/cmu-mocap/master/data/{subject:03d}/{take}.bvh"


def take_file(take):
    """A take by its number is fetched once and kept; anything else is a file already here."""
    if not re.fullmatch(r"\d+_\d+", take):
        return take
    path = os.path.join(".work", "cmu", take + ".bvh")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        request = urllib.request.Request(MIRROR.format(subject=int(take.split("_")[0]), take=take), headers={"User-Agent": "motion-fetch/1.0"})
        data = urllib.request.urlopen(request, timeout=60).read()
        if not data.lstrip().startswith(b"HIERARCHY"):
            raise SystemExit(f"take {take}: what came back is not a BVH file")
        with open(path, "wb") as f:
            f.write(data)
    return path


def read_bvh(path):
    """The take: each joint's parent and channels, and every frame's world turn of every
    joint and the hips' place."""
    tokens = open(path).read().split()
    joints, stack, i = [], [], 0
    while tokens[i] != "MOTION":
        t = tokens[i]
        if t in ("ROOT", "JOINT"):
            joints.append({"name": tokens[i + 1], "parent": stack[-1] if stack else None, "channels": []})
            stack.append(len(joints) - 1)
            i += 2
        elif t == "End":
            stack.append(None)
            i += 2
        elif t == "CHANNELS":
            n = int(tokens[i + 1])
            joints[stack[-1]]["channels"] = tokens[i + 2:i + 2 + n]
            i += 2 + n
        elif t == "}":
            stack.pop()
            i += 1
        else:
            i += 1
    frames = int(tokens[tokens.index("Frames:", i) + 1])
    at = tokens.index("Time:", i)
    step = float(tokens[at + 1])
    values = [float(v) for v in tokens[at + 2:]]
    width = sum(len(j["channels"]) for j in joints)
    turn = {"Xrotation": "X", "Yrotation": "Y", "Zrotation": "Z"}
    out = []
    for f in range(frames):
        row, k, world, place = values[f * width:(f + 1) * width], 0, [], Vector((0, 0, 0))
        for j in joints:
            local = Matrix.Identity(3)
            for ch in j["channels"]:
                v = row[k]
                k += 1
                if ch in turn:
                    local = local @ Matrix.Rotation(math.radians(v), 3, turn[ch])
                elif j["parent"] is None:
                    place["XYZ".index(ch[0])] = v
            world.append(local if j["parent"] is None else world[j["parent"]] @ local)
        out.append(({j["name"]: world[n] for n, j in enumerate(joints)}, place.copy()))
    return out, step


def cycle(frames, step, lo, hi):
    """The two frames between `lo` and `hi` seconds apart whose poses are most alike, away
    from the take's ends: one cycle of a gait lies between them."""
    legs = ["LeftUpLeg", "LeftLeg", "RightUpLeg", "RightLeg", "LowerBack", "LeftArm", "RightArm"]
    pose = [[(frames[f][0]["Hips"].inverted() @ frames[f][0][b]).to_quaternion() for b in legs] for f in range(len(frames))]
    best = (1e9, 0, len(frames) - 1)
    for a in range(int(0.15 / step), len(frames), 2):
        for b in range(a + int(lo / step), min(a + int(hi / step), len(frames) - int(0.1 / step)), 2):
            apart = sum(pose[a][i].rotation_difference(pose[b][i]).angle for i in range(len(legs))) + abs(frames[a][1].y - frames[b][1].y) * 0.2
            if apart < best[0]:
                best = (apart, a, b)
    return best[1], best[2]


def main():
    args = sys.argv[1:]
    source, out, clips = args[0], args[1], [a.split("=", 1) for a in args[2:]]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=source)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    # (The importer's own picture of a bone is not part of the creature.)
    for o in [o for o in bpy.data.objects if o.type == "MESH" and not o.find_armature()]:
        bpy.data.objects.remove(o)
    prefix = next(b.name[:-len("Hips")] for b in arm.data.bones if b.name.endswith("Hips"))
    rest = {b.name: b.matrix_local.to_3x3() for b in arm.data.bones}
    order = [b for b in arm.data.bones]  # parents come before their children
    hips = arm.data.bones[prefix + "Hips"]
    bpy.context.scene.render.fps = FPS
    speeds = {}
    if arm.animation_data is None:
        arm.animation_data_create()
    for name, take in clips:
        path, _, span = take.partition(":")
        path = take_file(path)
        frames, step = read_bvh(path)
        a, b = (0, len(frames) - 1)
        if span:
            lo, hi = (float(v) for v in span.split("-"))
            a, b = cycle(frames, step, lo, hi)
        picked = [frames[min(b, a + round(i / FPS / step))] for i in range(int((b - a) * step * FPS) + 1)]
        # Its size beside the take's: by how high the hips ride.
        scale = hips.head_local.z / (sum(p.y for _, p in picked) / len(picked))
        # The hips' travel over the ground, as a line from the first frame to the last, is taken out.
        first, last = picked[0][1], picked[-1][1]
        for old in [x for x in bpy.data.actions if x.name == name]:
            old.name = "tripo_" + name
        action = bpy.data.actions.new(name)
        arm.animation_data.action = action
        for i, (world, place) in enumerate(picked):
            posed = {}
            for bone in order:
                pb = arm.pose.bones[bone.name]
                parent = posed[bone.parent.name] @ rest[bone.parent.name].inverted() @ rest[bone.name] if bone.parent else rest[bone.name]
                short = bone.name[len(prefix):]
                if short in BONES and BONES[short] in world:
                    want = AXES @ world[BONES[short]] @ AXES.inverted() @ rest[bone.name]
                    pb.rotation_mode = "QUATERNION"
                    pb.rotation_quaternion = (parent.inverted() @ want).to_quaternion()
                    pb.keyframe_insert("rotation_quaternion", frame=i + 1)
                    posed[bone.name] = want
                else:
                    posed[bone.name] = parent
            t = i / max(len(picked) - 1, 1)
            moved = place - first.lerp(last, t)
            moved.y = place.y - sum(p.y for _, p in picked) / len(picked)
            pb = arm.pose.bones[hips.name]
            pb.location = rest[hips.name].inverted() @ (AXES @ moved * scale)
            pb.keyframe_insert("location", frame=i + 1)
        # How fast the one who was recorded was going, at the creature's size: the speed the
        # clip's feet hold the ground at. Forge is told, and need not guess it from the feet.
        over = (last - first)
        over.y = 0.0
        speeds[name] = over.length * scale / max((b - a) * step, 1e-6)
        print(f"retarget {name}: {path.rsplit('/', 1)[-1]} frames {a}..{b} of {len(frames)} ({(b - a) * step:.2f} s) -> {len(picked)} frames at {FPS} a second, size x{scale:.4f}, {speeds[name]:.3f} units a second")
    for o in bpy.data.objects:
        o.select_set(True)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_animations=True, export_animation_mode="ACTIONS", export_force_sampling=True, export_yup=True, export_skins=True, use_selection=False)
    # Beside it: each clip's speed over the ground, in the file's units a second.
    with open(out.rsplit(".", 1)[0] + ".toml", "w") as f:
        f.write("# Written by tools/blender/retarget.py: how fast each captured clip goes, file units a second.\n")
        for name, speed in speeds.items():
            f.write(f"{name} = {speed:.4f}\n")
    print(f"ok retarget: {len(clips)} clips -> {out}")


main()
