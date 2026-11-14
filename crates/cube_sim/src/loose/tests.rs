use super::*;

/// A floor of stone far wider than [`HELD`] cells, its top at y = 0.
fn floor() -> World {
    let mut w = World::default();
    w.fill(Vec3::new(-12.0, -1.0, -12.0), Vec3::new(12.0, 0.0, 12.0), Block::Stone);
    w
}

fn run(loose: &mut Loose, w: &mut World) -> f32 {
    let mut t = 0.0;
    while !loose.pieces.is_empty() && t < 30.0 {
        loose.step(w, crate::DT);
        t += crate::DT;
    }
    t
}

fn count(w: &World, lo: Vec3, hi: Vec3, b: Block) -> usize {
    let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
    (lo.y..hi.y).flat_map(|y| (lo.z..hi.z).flat_map(move |z| (lo.x..hi.x).map(move |x| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == b).count()
}

/// The cells of rubble in a box: what lies in the grid and what lies at rest as itself
/// ([`Rested`]: its stone is its own, and the world has only where it is, to the bit).
fn rubble(w: &World, loose: &Loose, lo: Vec3, hi: Vec3) -> usize {
    let own = loose.rested.iter().flat_map(|r| r.cells.keys().map(move |c| r.now((c.as_vec3() + 0.5) * CELL))).filter(|p| p.cmpge(lo).all() && p.cmplt(hi).all()).count();
    count(w, lo, hi, Block::Shard) + own
}

#[test]
fn digging_a_hole_in_the_ground_looses_nothing() {
    let mut w = floor();
    let mut loose = Loose::default();
    loose.cut(&mut w, Vec3::new(0.0, -0.1, 0.0), 0.4, Vec3::X, &[], None);
    assert!(loose.pieces.is_empty(), "the land came loose: {} pieces", loose.pieces.len());
}

#[test]
fn a_pole_cut_through_goes_over_and_lies_on_the_ground() {
    let mut w = floor();
    w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
    let wood = count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber);
    let mut loose = Loose::default();
    loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::X, &[], None);
    assert_eq!(loose.pieces.len(), 1, "what was over the cut did not come loose");
    let (cells, standing) = (loose.pieces[0].cells.len(), count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber));
    assert!(cells > 60 && standing < wood - cells + 1, "the piece has {cells} cells and {standing} of {wood} are left standing");
    let took = run(&mut loose, &mut w);
    let down = loose.landed[0];
    // It went over (a second or two, not at once and not for ever), it lies along the
    // way it was pushed and low, and nearly all of it is there.
    assert!((0.6..6.0).contains(&took), "it took {took:.1} s to come down");
    let lying = count(&w, Vec3::new(0.5, 0.0, -1.0), Vec3::new(8.0, 1.25, 1.5), Block::Timber);
    let high = count(&w, Vec3::new(-2.0, 2.0, -2.0), Vec3::new(8.0, 12.0, 2.0), Block::Timber);
    assert!(lying as f32 > cells as f32 * 0.8 && high == 0, "{lying} of {cells} cells lie along the ground, {high} are still up · {down:?}");
}

#[test]
fn what_is_dug_out_from_under_drops_straight_and_stays_whole() {
    let mut w = floor();
    // A slab held up at one end by a post.
    w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 2.0, 0.5), Block::Stone);
    w.fill(Vec3::new(0.0, 2.0, 0.0), Vec3::new(3.0, 2.5, 1.0), Block::Stone);
    let mut loose = Loose::default();
    // (The whole post taken away: left a stub of it, the slab comes to rest on that.)
    w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 2.0, 0.5), Block::Air);
    loose.loosen(&mut w, IVec3::new(-1, -1, -1), IVec3::new(3, 9, 3), Vec3::Z);
    assert_eq!(loose.pieces.len(), 1);
    let cells = loose.pieces[0].cells.len();
    run(&mut loose, &mut w);
    // (Come loose, stone is rubble.)
    let (low, up) = (rubble(&w, &loose, Vec3::new(-1.0, 0.0, -1.0), Vec3::new(4.0, 1.5, 2.0)), rubble(&w, &loose, Vec3::new(-1.0, 1.75, -1.0), Vec3::new(4.0, 4.0, 2.0)));
    assert!(low as f32 > cells as f32 * 0.85 && up == 0, "{low} of {cells} cells are down, {up} are still up");
}

#[test]
fn a_tree_that_is_cut_into_is_cells_from_then_on_and_one_whose_limbs_touch_it_is_not() {
    use crate::plants::{Clump, Limb};
    let mut w = floor();
    // Two trees as `plants` notes them, a limb of each reaching into the other.
    let mut form = Form::default();
    for (tree, x) in [(1, 0.0f32), (2, 3.0)] {
        let reach = if tree == 1 { 2.0 } else { -2.0 };
        for limb in [Limb { a: Vec3::new(x, 0.0, 0.0), b: Vec3::new(x, 5.0, 0.0), ra: 0.25, rb: 0.2, bark: Block::Wood, roots: 0, tree }, Limb { a: Vec3::new(x, 3.0, 0.0), b: Vec3::new(x + reach, 4.0, 0.0), ra: 0.12, rb: 0.1, bark: Block::Wood, roots: 0, tree }] {
            limb.fill(CELL, |c| w.set(c, Block::Wood));
            form.limbs.push(limb);
        }
        w.ellipsoid(Vec3::new(x, 5.5, 0.0), Vec3::splat(1.2), Block::Leaves, true);
        form.clumps.push(Clump { middle: Vec3::new(x, 5.5, 0.0), radii: Vec3::splat(1.2), leaf: Block::Leaves, tree });
    }
    let count_all = |w: &World, b: Block| count(w, Vec3::splat(-12.0), Vec3::splat(12.0), b);
    let (wood, leaves) = (count_all(&w, Block::Wood), count_all(&w, Block::Leaves));
    let mut loose = Loose::default();
    // A notch, not a cut through: it stands, and all of it is wrought and the other is not.
    let cut = loose.cut(&mut w, Vec3::new(-0.3, 1.0, 0.0), 0.2, Vec3::X, &[&form], None);
    assert!(loose.pieces.is_empty(), "a notch felled it");
    assert_eq!(cut.trees, vec![(0, 1)]);
    let (timber, brush) = (count_all(&w, Block::Timber), count_all(&w, Block::Brush));
    assert!(timber > wood / 3 && brush > leaves / 3, "timber {timber} of {wood} wood, brush {brush} of {leaves} leaf");
    assert!(count_all(&w, Block::Wood) > wood / 3 && count_all(&w, Block::Leaves) > leaves / 3, "the tree beside it was made wrought too");
    // Cut through, it comes down though the other's limb is in its crown.
    for step in -2..=2 {
        loose.cut(&mut w, Vec3::new(step as f32 * 0.2, 1.0, 0.0), 0.4, Vec3::NEG_X, &[&form], None);
    }
    assert_eq!(loose.pieces.len(), 1, "cut through, it hangs by its neighbour");
    // And it is still known as a tree: its shape, every bite, and (once down) where it lies.
    run(&mut loose, &mut w);
    assert_eq!(loose.things.len(), 1);
    let thing = &loose.things[0];
    assert!(thing.limbs.len() == 2 && thing.clumps.len() == 1 && thing.cuts.len() >= 2, "{} limbs, {} clumps, {} cuts", thing.limbs.len(), thing.clumps.len(), thing.cuts.len());
    let (_, _, turn) = thing.lies.expect("it came down and is not noted as lying");
    assert!((turn * Vec3::Y).y < 0.35, "noted as lying, but still upright: its up is {:?}", turn * Vec3::Y);
}

#[test]
fn a_crumb_of_a_log_that_nothing_holds_does_not_let_the_log_go() {
    let mut w = floor();
    let mut loose = Loose::default();
    let cells: Cells<Part> = (0..12).flat_map(|x| (0..2).flat_map(move |y| (0..2).map(move |z| (IVec3::new(x, 4 + y, z), Part::Whole(Block::Balk))))).collect();
    let piece = loose.made(cells, Vec3::new(1.5, 1.25, 0.25), Quat::from_rotation_y(0.4));
    let id = piece.id;
    loose.pieces.push(piece);
    run(&mut loose, &mut w);
    assert!(loose.pieces.is_empty() && loose.rested.len() == 1);
    // One bit of it apart from the rest, in the air (as where a bush left room for one),
    // and the place looked at again.
    let crumb = IVec3::new(2, 6, 9);
    w.set_bit(crumb * SPLIT, Block::Log);
    loose.owner.insert(crumb, id);
    loose.rested[0].placed.push(crumb);
    loose.loosen(&mut w, crumb - 1, crumb + 1, Vec3::X);
    assert!(loose.pieces.is_empty() && loose.rested.len() == 1, "the log came loose for a crumb of it: {} falling", loose.pieces.len());
    // Leaf gives way to what comes to rest in it.
    let mut w = floor();
    w.fill(Vec3::new(0.0, 0.0, -1.0), Vec3::new(3.0, 0.75, 1.0), Block::Leaves);
    let mut loose = Loose::default();
    let cells: Cells<Part> = (0..12).flat_map(|x| (0..2).flat_map(move |y| (0..2).map(move |z| (IVec3::new(x, 8 + y, z), Part::Whole(Block::Balk))))).collect();
    let piece = loose.made(cells, Vec3::new(1.5, 2.25, 0.25), Quat::from_rotation_y(0.4));
    let wood = piece.cells.len() * 125;
    loose.pieces.push(piece);
    run(&mut loose, &mut w);
    let there: usize = w.chunks.values().map(|c| (0..cube_voxel::CHUNK.pow(3)).map(|i| IVec3::new(i % 32, i / 1024, i / 32 % 32)).map(|l| match c.get(l) { Block::Broken => c.bits(l).map_or(0, |b| b.iter().filter(|b| b.whole()).count()), b if b.whole() => 125, _ => 0 }).sum::<usize>()).sum();
    assert!(there * 10 > wood * 9, "{there} of {wood} bits of a log that came down in a bush are in the world");
}

#[test]
fn one_of_two_logs_that_lie_against_each_other_is_taken_up_and_the_other_is_whole() {
    let mut w = floor();
    let mut loose = Loose::default();
    let log = |x: i32| -> Cells<Part> { (0..12).flat_map(|i| (0..2).flat_map(move |y| (0..2).map(move |z| (IVec3::new(x + i, 4 + y, z), Part::Whole(Block::Balk))))).collect() };
    let bits = |w: &World| -> usize { w.chunks.values().map(|c| (0..cube_voxel::CHUNK.pow(3)).map(|i| IVec3::new(i % 32, i / 1024, i / 32 % 32)).map(|l| match c.get(l) { Block::Broken => c.bits(l).map_or(0, |b| b.iter().filter(|b| b.whole()).count()), b if b.whole() => 125, _ => 0 }).sum::<usize>()).sum() };
    // One along the ground at an angle to the grid; then another beside it and at
    // another angle, so that they lie in some of the same cells.
    let first = loose.made(log(0), Vec3::new(1.5, 1.25, 0.25), Quat::from_rotation_y(0.3));
    loose.pieces.push(first);
    run(&mut loose, &mut w);
    let alone = bits(&w);
    let second = loose.made(log(0), Vec3::new(1.5, 1.25, 0.25), Quat::from_rotation_y(0.3) * Quat::from_rotation_y(0.12));
    let id = second.id;
    loose.pieces.push(second);
    loose.pieces[0].shift = Vec3::new(0.0, 0.0, 0.55);
    run(&mut loose, &mut w);
    assert_eq!(loose.rested.len(), 2);
    let shared = loose.rested[0].placed.iter().filter(|c| loose.rested[1].placed.contains(c)).count();
    assert!(shared > 0 && bits(&w) > alone, "the two do not lie in any of the same cells");
    // The second taken out of the world: the first is all there, and every cell of it its own.
    let second = loose.rested.remove(loose.rested.iter().position(|r| r.id == id).unwrap());
    loose.unrest(&mut w, &second, &mut HashSet::new());
    // (But for what of it was in the other: they may lie a little in each other, `SUNK`.)
    let left = bits(&w);
    assert!(left <= alone && left * 100 >= alone * 97, "the log left lying has {left} of its {alone} bits");
    let first = &loose.rested[0];
    assert!(first.placed.iter().all(|c| loose.owner.get(c) == Some(&first.id)) && !loose.owner.values().any(|o| *o == id), "a cell of the one left is not said to be its");
}

#[test]
fn a_light_thing_at_rest_is_shoved_along_and_a_heavy_one_is_not() {
    let mut w = floor();
    let mut loose = Loose::default();
    let log = |wide: i32| -> Cells<Part> { (0..8).flat_map(|z| (0..wide).flat_map(move |y| (0..wide).map(move |x| (IVec3::new(x, 4 + y, z), Part::Whole(Block::Balk))))).collect() };
    for (wide, moves) in [(1, true), (3, false)] {
        let piece = loose.made(log(wide), Vec3::new(0.125, 1.0, 1.0), Quat::IDENTITY);
        let (id, weighs) = (piece.id, piece.mass);
        loose.pieces.push(piece);
        run(&mut loose, &mut w);
        let lies = |loose: &Loose| loose.rested.iter().find(|r| r.id == id).map(|r| r.now(r.pivot));
        let was = lies(&loose).expect("it came to rest");
        // Heaved four times.
        let mut heaved = 0;
        for _ in 0..4 {
            let at = loose.rested.iter().find(|r| r.id == id).map(|r| (r.placed[0].as_vec3() + 0.5) * CELL).unwrap();
            heaved += loose.shove(&mut w, at, Vec3::X).is_some() as u32;
            run(&mut loose, &mut w);
        }
        let went = lies(&loose).expect("it lies again").x - was.x;
        if moves {
            assert!(heaved == 4 && (0.3..3.0).contains(&went), "a log of {weighs:.0} kg leaned on four times went {went:.2} m in {heaved} heaves");
        } else {
            assert!(heaved == 0 && went == 0.0, "a log of {weighs:.0} kg was shoved {went:.2} m");
        }
        let all = std::mem::take(&mut loose.rested);
        for r in &all {
            loose.unrest(&mut w, r, &mut HashSet::new());
        }
    }
}

#[test]
fn what_comes_down_on_another_piece_in_the_air_does_not_go_through_it() {
    let mut w = floor();
    let mut loose = Loose::default();
    let block = |y: i32| -> Cells<Part> { (0..2).flat_map(|x| (0..2).flat_map(move |dy| (0..2).map(move |z| (IVec3::new(x, y + dy, z), Part::Whole(Block::Balk))))).collect() };
    // One let go eight metres up; another right over it, already dropping fast.
    let (lower, mut upper) = (loose.made(block(32), Vec3::new(0.25, 8.25, 0.25), Quat::IDENTITY), loose.made(block(35), Vec3::new(0.25, 9.0, 0.25), Quat::IDENTITY));
    upper.vel.y = -7.0;
    let (low, high) = (lower.id, upper.id);
    loose.pieces.extend([lower, upper]);
    let (mut least, mut together) = (f32::MAX, 0);
    for _ in 0..(8.0 / crate::DT) as usize {
        loose.step(&mut w, crate::DT);
        let at = |id: u32| loose.pieces.iter().find(|p| p.id == id).map(|p| p.now(p.pivot).y);
        if let (Some(a), Some(b)) = (at(low), at(high)) {
            (least, together) = (least.min(b - a), together + 1);
        }
    }
    // (Each is half a metre high: nearer than that, middle to middle, and one is in the other.)
    assert!(together > 10 && least > 0.4, "over {together} ticks in the air together the upper came within {least:.2} m of the lower");
    assert!(loose.pieces.is_empty() && loose.rested.len() == 2);
    let lies = |id: u32| loose.rested.iter().find(|r| r.id == id).map(|r| r.now(r.pivot).y).unwrap();
    assert!(lies(high) > lies(low) + 0.3, "the upper lies at {:.2} and the lower at {:.2}", lies(high), lies(low));
}

#[test]
fn stone_that_drops_shatters_into_rubble_and_wood_does_not() {
    for (block, breaks) in [(Block::Stone, true), (Block::Timber, false)] {
        let mut w = floor();
        w.fill(Vec3::new(0.0, 3.0, 0.0), Vec3::new(2.0, 4.5, 2.0), block);
        let mut loose = Loose::default();
        loose.loosen(&mut w, IVec3::new(-1, 11, -1), IVec3::new(9, 19, 9), Vec3::X);
        assert_eq!(loose.pieces.len(), 1);
        let cells = loose.pieces[0].cells.len();
        let mut most = 1;
        let mut t = 0.0;
        while !loose.pieces.is_empty() && t < 30.0 {
            loose.step(&mut w, crate::DT);
            most = most.max(loose.pieces.len());
            t += crate::DT;
        }
        let lies = |lo: Vec3, hi: Vec3| if block == Block::Stone { rubble(&w, &loose, lo, hi) } else { count(&w, lo, hi, block) };
        let down = lies(Vec3::new(-8.0, 0.0, -8.0), Vec3::new(10.0, 3.0, 10.0));
        let block_shape = lies(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 1.5, 2.0));
        if breaks {
            // Many lumps, thrown wider than the block was, most of it still there, all at rest.
            assert!(most >= 8, "it broke into {most}");
            assert!(down as f32 > cells as f32 * 0.6 && (block_shape as f32) < cells as f32 * 0.8, "{down} of {cells} cells came down, {block_shape} of them where the block fell");
        } else {
            assert!(most == 1 && block_shape as f32 > cells as f32 * 0.9, "wood broke into {most}: {block_shape} of {cells} where it fell");
        }
        assert!(loose.pieces.is_empty(), "something never came to rest");
    }
}

#[test]
fn a_tool_cuts_its_own_stuff_and_leaves_the_rest() {
    let mut w = floor();
    w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 3.0, 0.5), Block::Timber);
    let mut loose = Loose::default();
    // An axe at the foot of a post: wood goes, the stone under it does not.
    let cut = loose.cut(&mut w, Vec3::new(0.25, 0.0, 0.25), 0.4, Vec3::X, &[], Some(Stuff::Wood));
    assert!(cut.taken[Stuff::Wood as usize] > 100 && cut.taken[Stuff::Stone as usize] == 0, "took {:?}", cut.taken);
    assert!(w.get(IVec3::new(0, -1, 0)) == Block::Stone, "an axe cut stone");
    // And a pick there takes the stone and leaves what wood is left.
    let wood = count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber);
    let cut = loose.cut(&mut w, Vec3::new(1.0, 0.0, 0.25), 0.4, Vec3::X, &[], Some(Stuff::Stone));
    assert!(cut.taken[Stuff::Stone as usize] > 100 && cut.taken[Stuff::Wood as usize] == 0, "took {:?}", cut.taken);
    assert_eq!(count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber), wood);
}

#[test]
fn water_runs_into_a_hole_dug_beside_it_and_no_higher_than_it_stood() {
    let mut w = floor();
    // A pool in the floor, and a wall of earth between it and a dry pit, lower.
    w.fill(Vec3::new(0.0, -1.0, 0.0), Vec3::new(2.0, 0.0, 2.0), Block::Water);
    w.fill(Vec3::new(2.5, -1.0, 0.0), Vec3::new(4.5, 0.0, 2.0), Block::Air);
    assert_eq!(count(&w, Vec3::splat(-6.0), Vec3::splat(6.0), Block::Water), 256);
    let mut loose = Loose::default();
    loose.cut(&mut w, Vec3::new(2.25, -0.5, 1.0), 0.45, Vec3::X, &[], None);
    let pit = count(&w, Vec3::new(2.5, -1.0, 0.0), Vec3::new(4.5, 0.0, 2.0), Block::Water);
    let over = count(&w, Vec3::new(-6.0, 0.0, -6.0), Vec3::new(6.0, 4.0, 6.0), Block::Water);
    assert!(pit == 256 && over == 0, "{pit} of 256 cells of the pit filled, {over} cells of water above where it stood");
}

#[test]
fn a_trunk_lying_on_the_ground_is_cut_into_lengths_that_stay_where_they_lie() {
    let mut w = floor();
    w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
    let mut loose = Loose::default();
    loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::X, &[], Some(Stuff::Wood));
    run(&mut loose, &mut w);
    // Where it lies: flat on the ground (none of it more than its own thickness up), from
    // one end to the other.
    let across = |w: &World, x: f32| count(w, Vec3::new(x - 0.1, 0.0, -1.0), Vec3::new(x + 0.1, 1.25, 1.5), Block::Timber);
    let along: Vec<f32> = (0..60).map(|i| 0.9 + i as f32 * 0.25).filter(|x| across(&w, *x) > 0).collect();
    let (from, to) = (along[0], along[along.len() - 1]);
    let (lying, up) = (count(&w, Vec3::new(0.8, 0.0, -1.0), Vec3::new(16.0, 0.75, 1.5), Block::Timber), count(&w, Vec3::new(0.8, 0.75, -1.0), Vec3::new(16.0, 8.0, 1.5), Block::Timber));
    assert!(to - from > 3.5 && up == 0 && lying > 60, "it lies from {from:.1} to {to:.1} m, {lying} cells low and {up} higher than its own thickness");
    // Two cuts across it with an axe, chop by chop, across it and down through it: three
    // lengths, each left where it lay, the wood cut away carried off and none of the
    // stone under it touched.
    let (first, second) = (from + (to - from) * 0.35, from + (to - from) * 0.68);
    let mut taken = 0;
    for x in [first, second] {
        for (y, z) in [(0.6, 0.0), (0.6, 0.25), (0.6, 0.5), (0.3, 0.0), (0.3, 0.25), (0.3, 0.5)] {
            let cut = loose.cut(&mut w, Vec3::new(x, y, z), 0.28, Vec3::X, &[], Some(Stuff::Wood));
            assert_eq!(cut.taken[Stuff::Stone as usize], 0, "an axe cut the ground under the log");
            taken += cut.taken[Stuff::Wood as usize];
        }
    }
    assert!(loose.pieces.is_empty(), "a length cut from a lying trunk came loose: {} pieces", loose.pieces.len());
    let thirds = [(from + first) / 2.0, (first + second) / 2.0, (second + to) / 2.0].map(|x| across(&w, x));
    assert!(across(&w, first) == 0 && across(&w, second) == 0 && thirds.iter().all(|n| *n > 0), "not three lengths: wood at the cuts {} and {}, between them {thirds:?}", across(&w, first), across(&w, second));
    let left = count(&w, Vec3::new(0.8, 0.0, -1.0), Vec3::new(16.0, 1.25, 1.5), Block::Timber);
    assert!(taken > 200 && left < lying && left as f32 > lying as f32 * 0.6, "took {taken} bits; {left} of {lying} cells of it left");
}

#[test]
fn a_trunk_cut_most_of_the_way_through_snaps_and_one_only_notched_stands() {
    for (bites, falls) in [(1, false), (3, true)] {
        let mut w = floor();
        // A trunk a metre through, and chops in one line into its middle: a hole through
        // it, with wood left standing either side.
        w.fill(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
        let mut loose = Loose::default();
        for n in 0..bites {
            loose.cut(&mut w, Vec3::new(-0.4 + n as f32 * 0.4, 1.0, 0.0), 0.36, Vec3::X, &[], Some(Stuff::Wood));
        }
        assert_eq!(!loose.pieces.is_empty(), falls, "after {bites} chops into a trunk a metre through");
    }
}

#[test]
fn earth_let_go_heaps_and_stone_stays_on_the_ground_and_falls_from_the_air() {
    let mut w = floor();
    let mut loose = Loose::default();
    // Twenty lumps let go at one place two metres up: a heap on the ground, low and
    // wide, and nothing left in the air.
    for _ in 0..20 {
        loose.put(&mut w, Vec3::new(0.1, 2.0, 0.1), 0.155, Block::Dirt);
    }
    let earth: Vec<IVec3> = (-12..12).flat_map(|x| (-12..12).flat_map(move |z| (0..16).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) != Block::Air).collect();
    let (top, wide) = (earth.iter().map(|c| c.y).max().unwrap(), earth.iter().map(|c| c.x.abs().max(c.z.abs())).max().unwrap());
    assert!(top <= 3 && wide >= 2, "a heap {top} cells high and {wide} out, of {} cells", earth.len());
    // A stone put on the ground stays as it was put, which is building; one let go in
    // the air comes down, and from two metres it breaks.
    loose.put(&mut w, Vec3::new(4.1, 0.1, 0.1), 0.155, Block::Stone);
    assert!(loose.pieces.is_empty() && w.get(IVec3::new(16, 0, 0)) != Block::Air, "a stone on the ground did not stay");
    loose.put(&mut w, Vec3::new(8.1, 2.0, 0.1), 0.5, Block::Stone);
    assert_eq!(loose.pieces.len(), 1, "a stone let go in the air hangs there");
    run(&mut loose, &mut w);
    assert_eq!(w.get(IVec3::new(32, 8, 0)), Block::Air, "it is still in the air");
    assert!(loose.landed.iter().any(|l| l.speed > 4.5), "it did not come down hard: {:?}", loose.landed.iter().map(|l| l.speed).collect::<Vec<_>>());
}

#[test]
fn the_same_edits_done_again_give_the_same_world() {
    use crate::plants::{Clump, Limb};
    let grown = || {
        let mut w = floor();
        let mut form = Form::default();
        let limb = Limb { a: Vec3::new(0.0, 0.0, 0.0), b: Vec3::new(0.2, 6.0, 0.0), ra: 0.3, rb: 0.2, bark: Block::Wood, roots: 0, tree: 1 };
        limb.fill(CELL, |c| w.set(c, Block::Wood));
        form.limbs.push(limb);
        w.ellipsoid(Vec3::new(0.2, 6.5, 0.0), Vec3::splat(1.5), Block::Leaves, true);
        form.clumps.push(Clump { middle: Vec3::new(0.2, 6.5, 0.0), radii: Vec3::splat(1.5), leaf: Block::Leaves, tree: 1 });
        (w, form)
    };
    // Played: a tree cut down, a hole dug while it is still falling, earth put down
    // after, and the fallen trunk cut across.
    let (mut w, form) = grown();
    let mut loose = Loose::default();
    for x in [-0.3, 0.0, 0.3] {
        loose.cut(&mut w, Vec3::new(x, 1.0, 0.0), 0.36, Vec3::X, &[&form], Some(Stuff::Wood));
    }
    for _ in 0..40 {
        loose.step(&mut w, crate::DT);
    }
    loose.cut(&mut w, Vec3::new(-3.0, -0.1, 2.0), 0.5, Vec3::X, &[&form], None);
    run(&mut loose, &mut w);
    loose.put(&mut w, Vec3::new(-3.0, 0.2, -2.0), 0.3, Block::Dirt);
    loose.cut(&mut w, Vec3::new(3.0, 0.4, 0.0), 0.4, Vec3::X, &[&form], Some(Stuff::Wood));
    run(&mut loose, &mut w);
    // Done again to the land as it grew.
    let (mut again, form) = grown();
    replay(&mut again, &[&form], &loose.journal);
    let (lo, hi) = (IVec3::splat(-48), IVec3::splat(48));
    let mut differ = 0;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                let c = IVec3::new(x, y, z);
                differ += (w.get(c) != again.get(c) || w.bits_at(c).map(|b| b.to_vec()) != again.bits_at(c).map(|b| b.to_vec())) as usize;
            }
        }
    }
    assert!(loose.journal.len() == 6 && differ == 0, "{} edits; {differ} cells differ", loose.journal.len());
    assert!(count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber) > 50, "nothing was felled to compare");
}

#[test]
fn two_crowns_that_touch_do_not_hold_each_other_up() {
    let mut w = floor();
    // (Both cut into already: each is cells.)
    for x in [0.0, 3.0] {
        w.fill(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 4.0, 0.5), Block::Timber);
        w.fill(Vec3::new(x - 1.25, 4.0, -1.0), Vec3::new(x + 1.75, 5.5, 1.5), Block::Brush);
    }
    let mut loose = Loose::default();
    loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::NEG_X, &[], None);
    assert_eq!(loose.pieces.len(), 1, "the cut tree hangs by its neighbour's leaves");
    // It took leaves with it, and left its neighbour its trunk.
    let piece = &loose.pieces[0];
    assert!(piece.cells.values().any(|p| matches!(p, Part::Whole(b) if b.foliage())), "it left all its leaves behind");
    assert!(w.get(IVec3::new(13, 8, 1)) == Block::Timber, "the neighbour's trunk went with it");
}

#[test]
fn a_tree_cut_through_falls_though_its_boughs_touch_another_that_was_cut_into() {
    use crate::plants::Limb;
    // Two trees a metre and a half apart, a bough of each reaching the other's trunk,
    // as `plants` grew them.
    let trees = || {
        let (mut w, mut form) = (floor(), Form::default());
        for (tree, x) in [(1, 0.0f32), (2, 1.5)] {
            let out = if tree == 1 { 1.5 } else { -1.5 };
            for (a, b, ra, roots) in [(Vec3::new(x, 0.0, 0.0), Vec3::new(x, 6.0, 0.0), 0.3, 1), (Vec3::new(x, 4.0, 0.0), Vec3::new(x + out, 4.5, 0.0), 0.12, 0)] {
                let limb = Limb { a, b, ra, rb: ra * 0.8, bark: Block::Wood, roots, tree };
                limb.fill(CELL, |c| w.set(c, Block::Wood));
                form.limbs.push(limb);
            }
        }
        (w, form)
    };
    let (mut w, form) = trees();
    let mut loose = Loose::default();
    // A bite that reaches both, which makes both cells; then the first cut through.
    loose.cut(&mut w, Vec3::new(0.75, 1.0, 0.0), 0.5, Vec3::NEG_X, &[&form], Some(Stuff::Wood));
    assert_eq!(loose.things.len(), 2, "both were to be cut into");
    for x in [-0.3, 0.0, 0.3] {
        loose.cut(&mut w, Vec3::new(x, 1.0, 0.0), 0.4, Vec3::NEG_X, &[&form], Some(Stuff::Wood));
    }
    assert_eq!(loose.pieces.len(), 1, "it hangs by a bough from the tree beside it");
    // And comes down to the ground, through the other, which stands.
    run(&mut loose, &mut w);
    let lies = loose.landed.iter().map(|l| l.at.y).fold(f32::MAX, f32::min);
    assert!(lies < 1.5, "it lies {lies} m up");
    assert_ne!(w.get(IVec3::new(6, 12, 0)), Block::Air, "the other tree went with it");
}

#[test]
fn a_wall_built_of_stone_stands_and_comes_down_when_its_foot_is_cut_away() {
    let mut w = floor();
    let mut loose = Loose::default();
    // Eight cells long and six high, a piece at a time, each put on what is there.
    for y in 0..6 {
        for x in 0..8 {
            loose.put(&mut w, (IVec3::new(x, y, 0).as_vec3() + 0.5) * CELL, 0.155, Block::Stone);
        }
    }
    run(&mut loose, &mut w);
    let standing = |w: &World| (0..6).flat_map(|y| (0..8).map(move |x| IVec3::new(x, y, 0))).filter(|c| has(w, *c, false)).count();
    assert!(loose.pieces.is_empty() && standing(&w) == 48, "{} of 48 cells stand, {} loose", standing(&w), loose.pieces.len());
    // A doorway picked through it leaves the wall standing over the gap.
    loose.cut(&mut w, Vec3::new(1.0, 0.25, 0.125), 0.3, Vec3::Z, &[], Some(Stuff::Stone));
    run(&mut loose, &mut w);
    assert!(standing(&w) >= 40 && w.get(IVec3::new(4, 5, 0)) == Block::Stone, "a hole brought it down: {} stand", standing(&w));
    // Its whole foot cut away, what was over it comes down, and its stone is on the ground.
    for x in 0..9 {
        loose.cut(&mut w, Vec3::new(x as f32 * CELL, 0.125, 0.125), 0.3, Vec3::Z, &[], Some(Stuff::Stone));
    }
    assert!(!loose.pieces.is_empty(), "the wall stands on nothing");
    run(&mut loose, &mut w);
    let top = (0..12).rev().find(|y| (-8..16).any(|x| (-8..8).any(|z| w.get(IVec3::new(x, *y, z)) != Block::Air))).unwrap_or(0);
    let stone: usize = (-12..20).flat_map(|x| (-12..12).flat_map(move |z| (0..12).map(move |y| IVec3::new(x, y, z)))).filter(|c| has(&w, *c, false)).count();
    assert!(top < 6 && stone >= 20, "it lies {top} cells high, {stone} cells of stone on the ground");
}

#[test]
fn a_tree_that_lies_comes_down_with_the_ground_dug_from_under_it_as_the_tree_it_is() {
    use crate::plants::{Clump, Limb};
    let (mut w, mut form) = (floor(), Form::default());
    // (Stone two metres deeper than the floor is, to dig a metre of.)
    w.fill(Vec3::new(-12.0, -3.0, -12.0), Vec3::new(12.0, -1.0, 12.0), Block::Stone);
    let trunk = Limb { a: Vec3::new(0.1, 0.0, 0.1), b: Vec3::new(0.1, 8.0, 0.1), ra: 0.25, rb: 0.2, bark: Block::Wood, roots: 1, tree: 1 };
    trunk.fill(CELL, |c| w.set(c, Block::Wood));
    form.limbs.push(trunk);
    let bough = Limb { a: Vec3::new(0.1, 6.0, 0.1), b: Vec3::new(0.1, 6.6, 1.7), ra: 0.06, rb: 0.05, bark: Block::Wood, roots: 0, tree: 1 };
    bough.fill(CELL, |c| w.set(c, Block::Wood));
    form.limbs.push(bough);
    w.ellipsoid(Vec3::new(0.1, 6.7, 1.7), Vec3::splat(0.9), Block::Leaves, true);
    form.clumps.push(Clump { middle: Vec3::new(0.1, 6.7, 1.7), radii: Vec3::splat(0.9), leaf: Block::Leaves, tree: 1 });
    let mut loose = Loose::default();
    for _ in 0..BLOWS(0.25) {
        loose.chop(&mut w, Vec3::new(-0.2, 1.2, 0.1), Vec3::X, &[&form]);
    }
    assert_eq!(loose.pieces.len(), 1, "cut through, it stands: {} things", loose.things.len());
    let took = run(&mut loose, &mut w);
    assert!(loose.things[0].lies.is_some(), "after {took:.1} s: {} pieces, things {:?}", loose.pieces.len(), loose.things.iter().map(|t| (t.id, t.blows, t.felled, t.limbs.len())).collect::<Vec<_>>());
    let lies = |loose: &Loose| { let (pivot, shift, turn) = loose.things[0].lies.expect("it did not come down"); pivot + shift + turn * (Vec3::new(0.1, 5.0, 0.1) - pivot) };
    let was = lies(&loose);
    // The ground it lies on, and a good way round it, taken a metre down.
    w.fill(Vec3::new(1.0, -1.0, -1.0), Vec3::new(11.0, 0.0, 5.0), Block::Air);
    loose.loosen(&mut w, IVec3::new(-8, -5, -12), IVec3::new(48, 8, 12), Vec3::X);
    run(&mut loose, &mut w);
    let now = lies(&loose);
    assert!((0.6..1.4).contains(&(was.y - now.y)), "its trunk lay at {:.2} and, the ground gone from under it, lies at {:.2}", was.y, now.y);
    // And what stops a body is where it is drawn: its wood is down there, none left above.
    let high = (-8..48).flat_map(|x| (-12..12).flat_map(move |z| (0..8).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c).wood() && (c.x > 6 || c.y > 3)).count();
    // (But for a bough's end that came to lie on the edge of the pit.)
    assert!(high < 6, "{high} cells of its wood are still where it lay before");
}

#[test]
fn an_axe_fells_a_tree_by_its_blows_then_limbs_it_then_breaks_it_into_logs() {
    use crate::plants::{Clump, Limb};
    let (mut w, mut form) = (floor(), Form::default());
    // A trunk 8 m tall and 50 cm through, three boughs and their leaf.
    let mut grow = |a: Vec3, b: Vec3, ra: f32, roots: u8, w: &mut World| {
        let limb = Limb { a, b, ra, rb: ra * 0.8, bark: Block::Wood, roots, tree: 1 };
        limb.fill(CELL, |c| w.set(c, Block::Wood));
        form.limbs.push(limb);
    };
    grow(Vec3::new(0.1, 0.0, 0.1), Vec3::new(0.1, 8.0, 0.1), 0.25, 1, &mut w);
    for (y, out) in [(4.0, 1.6f32), (5.5, -1.4), (7.0, 1.2)] {
        grow(Vec3::new(0.1, y, 0.1), Vec3::new(0.1, y + 0.6, 0.1 + out), 0.06, 0, &mut w);
        w.ellipsoid(Vec3::new(0.1, y + 0.7, 0.1 + out), Vec3::splat(0.9), Block::Leaves, true);
        form.clumps.push(Clump { middle: Vec3::new(0.1, y + 0.7, 0.1 + out), radii: Vec3::splat(0.9), leaf: Block::Leaves, tree: 1 });
    }
    let mut loose = Loose::default();
    // Standing: it takes as many blows as its girth asks, and stands until the last.
    let needs = BLOWS(0.25);
    assert!((3..=5).contains(&needs), "{needs} blows for a trunk 50 cm through");
    for blow in 1..=needs {
        assert!(loose.pieces.is_empty() && loose.landed.is_empty(), "it was down after {} blows of {needs}", blow - 1);
        loose.chop(&mut w, Vec3::new(-0.2, 1.2, 0.1), Vec3::X, &[&form]);
    }
    assert_eq!(loose.pieces.len(), 1, "cut through, it stands");
    run(&mut loose, &mut w);
    let tree = &loose.things[0];
    let (pivot, shift, turn) = tree.lies.expect("it did not come down");
    let now = |p: Vec3| pivot + shift + turn * (p - pivot);
    assert!(now(Vec3::new(0.1, 5.0, 0.1)).y < 1.0, "its trunk lies {} m up", now(Vec3::new(0.1, 5.0, 0.1)).y);
    // Fallen: blows along it knock off its boughs and leaf, and leave its trunk.
    let count = |w: &World, of: &dyn Fn(Block) -> bool| (-60..60).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| of(w.get(*c))).count();
    let trunk = count(&w, &|b| b.wood());
    assert!(count(&w, &|b| b.foliage()) > 20, "it has no leaf to lose");
    for blow in 0..6 {
        if loose.things[0].limbed {
            break;
        }
        loose.chop(&mut w, now(Vec3::new(0.1, 3.0 + blow as f32 * 1.2, 0.1)), Vec3::NEG_Y, &[&form]);
        run(&mut loose, &mut w);
    }
    assert!(loose.things[0].limbed && count(&w, &|b| b.foliage()) == 0, "{} cells of leaf left", count(&w, &|b| b.foliage()));
    assert!(!loose.shed.is_empty() && count(&w, &|b| b.wood()) * 10 > trunk * 7, "its trunk went with its boughs: {} of {trunk}", count(&w, &|b| b.wood()));
    // Bare, it is timber: a blow at what is longer than a log breaks it through, and a
    // blow at a log splits it into hunks. Blows all along it, until it is all hunks.
    let (after, mut breaks, mut splits) = (count(&w, &|b| b.wood()), 0, 0);
    for blow in 0..24 {
        let (wood, billets) = (count(&w, &|b| b.wood()), count(&w, &|b| b == Block::Billet));
        loose.chop(&mut w, now(Vec3::new(0.1, 1.5 + (blow * 5 % 13) as f32 * 0.5, 0.1)), Vec3::NEG_Y, &[&form]);
        run(&mut loose, &mut w);
        let split = count(&w, &|b| b == Block::Billet) > billets;
        (breaks, splits) = (breaks + (!split && count(&w, &|b| b.wood()) < wood) as u32, splits + split as u32);
    }
    assert!(breaks >= 1 && splits >= 2, "broken through {breaks} times and split {splits}");
    let (left, billets) = (count(&w, &|b| b.wood()), count(&w, &|b| b == Block::Billet));
    // (All of it but its stump, which stands where it stood.)
    let stump = (-4..=4).flat_map(|x| (-4..=4).flat_map(move |z| (0..8).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c).wood()).count();
    let (left, after) = (left - stump, after - stump);
    assert!(left * 10 < after, "{left} cells of wood left in logs, of {after}");
    // (Two or three hunks a log, `SPLITS`, and it was split at least twice.)
    assert!(billets % 8 == 0 && (4..=3 * splits as usize).contains(&(billets / 8)), "{} hunks from {splits} logs split", billets / 8);
    // Every cell of them is a hunk that can be found, taken up, and put down elsewhere.
    let cells = |w: &World| -> Vec<IVec3> { (-60..60).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Billet).collect() };
    let all = cells(&w);
    assert!(all.iter().all(|c| hunk_at(&w, *c).is_some()), "a cell of a hunk that is no hunk");
    assert!(all.iter().all(|c| c.y < 6), "a hunk left in the air at {:?}", all.iter().map(|c| c.y).max());
    let at = (all[0].as_vec3() + 0.5) * CELL;
    assert_eq!(loose.lift(&mut w, at).map(|l| l.0), Some(Block::Billet));
    run(&mut loose, &mut w);
    assert_eq!(cells(&w).len(), billets - 8);
    assert!(loose.lay(&mut w, Vec3::new(-5.1, 2.0, -5.1), Block::Billet).is_some());
    run(&mut loose, &mut w);
    let laid = hunk_at(&w, (Vec3::new(-5.1, 0.1, -5.1) / CELL).floor().as_ivec3());
    assert_eq!(laid.map(|l| l.0), Some(IVec3::new(-22, 0, -22)), "put down in the air, it came down onto the ground on its square");
    // Over a cell part filled with loose leaf it comes to rest too, on the ground under it.
    w.set_bit(IVec3::new(30, 2, 30) * SPLIT + IVec3::ONE, Block::Brush);
    assert_eq!(w.get(IVec3::new(30, 2, 30)), Block::Broken);
    assert!(loose.lay(&mut w, Vec3::new(7.6, 1.6, 7.6), Block::Billet).is_some());
    let steps = (0..600).take_while(|_| { loose.step(&mut w, crate::DT); !loose.pieces.is_empty() }).count();
    assert!(steps < 300, "a hunk over loose leaf was still falling after five seconds");
    assert_eq!(hunk_at(&w, IVec3::new(30, 1, 30)).map(|l| l.0), Some(IVec3::new(30, 0, 30)));
    let billets = billets + 8;
    // One put on it lies on it, and each is still its own hunk.
    assert!(loose.lay(&mut w, Vec3::new(-5.1, 0.1, -5.1), Block::Billet).is_some());
    run(&mut loose, &mut w);
    assert_eq!(hunk_at(&w, IVec3::new(-21, 3, -21)).map(|l| l.0), Some(IVec3::new(-22, 2, -22)));
    assert_eq!(cells(&w).len(), billets + 8);
}

#[test]
fn a_pick_chips_a_boulder_until_it_breaks_into_hunks_and_digs_the_lands_own_stone() {
    let (mut w, mut form) = (floor(), Form::default());
    let stone = crate::plants::Stone { middle: Vec3::new(0.1, 0.6, 0.1), radii: Vec3::new(1.0, 0.8, 0.9), turn: 0.0, faces: Vec::new(), kind: Block::Rock };
    w.ellipsoid(stone.middle, stone.radii, Block::Rock, true);
    form.stones.push(stone);
    let count = |w: &World, of: &dyn Fn(Block) -> bool| (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| of(w.get(*c))).count();
    // (Three cubic metres of it: 24 hunks' worth, less what is under the ground.)
    let whole = count(&w, &|b| b.grown() == Block::Rock || b == Block::Broken);
    let (mut loose, needs) = (Loose::default(), CHIPS(1.0));
    assert_eq!(needs, 4);
    for blow in 1..needs {
        assert!(loose.chip(&mut w, Vec3::new(0.1, 1.3, 0.1), Vec3::NEG_Y, &[&form]).is_some());
        run(&mut loose, &mut w);
        let left = count(&w, &|b| b.grown() == Block::Rock || b == Block::Broken);
        assert!(left * 10 > whole * 8 && count(&w, &|b| b == Block::Cobble) == 0, "after {blow} blows of {needs}, {left} cells of {whole}");
    }
    assert!(loose.chip(&mut w, Vec3::new(0.1, 1.1, 0.1), Vec3::NEG_Y, &[&form]).is_some());
    run(&mut loose, &mut w);
    let (left, cobbles) = (count(&w, &|b| b.grown() == Block::Rock), count(&w, &|b| b == Block::Cobble));
    assert!(left == 0 && (20..=24).contains(&(cobbles / 8)) && cobbles % 8 == 0, "{left} cells of boulder left, {cobbles} of hunks");
    // They lie on the ground and on one another, each one that can be taken up.
    let all: Vec<IVec3> = (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Cobble).collect();
    assert!(all.iter().all(|c| hunk_at(&w, *c).is_some()));
    assert!(all.iter().all(|c| c.y == 0 || w.get(*c - IVec3::Y).solid()), "a hunk of stone in the air");
    // Rubble: stone that came down and broke is loose stone, and a blow at a lump of it
    // makes hunks of it, to the nearest one.
    w.fill(Vec3::new(6.0, 0.0, 6.0), Vec3::new(6.75, 0.75, 7.0), Block::Shard);
    assert!(loose.chip(&mut w, Vec3::new(6.3, 0.7, 6.3), Vec3::NEG_Y, &[&form]).is_some());
    run(&mut loose, &mut w);
    assert_eq!(count(&w, &|b| b == Block::Shard), 0);
    let (cobbles, all) = (cobbles + 40, (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Cobble).collect::<Vec<IVec3>>());
    assert_eq!(all.len(), cobbles, "36 cells of rubble are four and a half hunks: five, to the nearest");
    // A hunk is no boulder and is not cut: a pick does nothing to it, nor to the ground.
    let top = (all.iter().max_by_key(|c| c.y).unwrap().as_vec3() + 0.5) * CELL;
    assert!(loose.chip(&mut w, top, Vec3::NEG_Y, &[&form]).is_none());
    loose.cut(&mut w, top, 0.3, Vec3::NEG_Y, &[&form], Some(Stuff::Stone));
    assert_eq!(count(&w, &|b| b == Block::Cobble), cobbles);
    // Stone dug from the land comes away as a hunk, a bite a hunk.
    w.fill(Vec3::new(-8.0, 0.0, -8.0), Vec3::new(-5.0, 2.0, -5.0), Block::Stone);
    loose.cut(&mut w, Vec3::new(-6.5, 1.9, -6.5), 0.3, Vec3::NEG_Y, &[&form], Some(Stuff::Stone));
    run(&mut loose, &mut w);
    let cobbles = cobbles + 8;
    assert_eq!(count(&w, &|b| b == Block::Cobble), cobbles, "a bite of the land's stone left no hunk");
    // And the same blows on the same land give the same land.
    let (mut again, mut form2) = (floor(), Form::default());
    again.fill(Vec3::new(-8.0, 0.0, -8.0), Vec3::new(-5.0, 2.0, -5.0), Block::Stone);
    again.ellipsoid(Vec3::new(0.1, 0.6, 0.1), Vec3::new(1.0, 0.8, 0.9), Block::Rock, true);
    again.fill(Vec3::new(6.0, 0.0, 6.0), Vec3::new(6.75, 0.75, 7.0), Block::Shard);
    form2.stones.push(form.stones[0].clone());
    replay(&mut again, &[&form2], &loose.journal);
    assert_eq!(count(&again, &|b| b == Block::Cobble), cobbles);
}
