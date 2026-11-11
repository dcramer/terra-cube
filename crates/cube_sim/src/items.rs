//! What a person carries: items, the pack they are kept in, and the bar of it that is to hand.
//! Rules, not drawing: what a blade does to a blow and what berries do to a wound are decided
//! here, and the game's bar and pack windows only show it.

/// A thing that can be carried.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Item {
    Torch,
    Sword,
    Shovel,
    Berries,
    /// Cut from what was killed.
    Meat,
}

impl Item {
    pub const ALL: [Item; 5] = [Item::Torch, Item::Sword, Item::Shovel, Item::Berries, Item::Meat];

    pub fn name(self) -> &'static str {
        match self {
            Item::Torch => "Torch",
            Item::Sword => "Short sword",
            Item::Shovel => "Shovel",
            Item::Berries => "Berries",
            Item::Meat => "Meat",
        }
    }

    /// How many lie in one slot.
    pub fn stack(self) -> u32 {
        match self {
            Item::Berries => 20,
            Item::Meat => 10,
            _ => 1,
        }
    }

    /// What a blow with it in hand takes, if it is a weapon (bare hands take 6).
    pub fn power(self) -> Option<f32> {
        match self {
            Item::Sword => Some(28.0),
            Item::Shovel => Some(12.0),
            Item::Torch => Some(9.0),
            Item::Berries | Item::Meat => None,
        }
    }

    /// Whether it lights the way while it is held.
    pub fn lights(self) -> bool {
        self == Item::Torch
    }

    /// Health given back by eating one.
    pub fn heals(self) -> Option<f32> {
        match self {
            Item::Berries => Some(15.0),
            Item::Meat => Some(35.0),
            _ => None,
        }
    }
}

/// What using the thing in hand did, for whoever must show it or carry it out.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Used {
    /// A blow was begun.
    Swing,
    /// One was eaten.
    Ate,
    /// The ground in front is to be dug.
    Dig,
    Nothing,
}

/// Slots in a pack, and how many of them (the first) are the bar to hand.
pub const SLOTS: usize = 24;
pub const BAR: usize = 8;

/// A pack: slots of items, the first [`BAR`] of them to hand, one of those in the hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    pub slots: [Option<(Item, u32)>; SLOTS],
    /// Which slot of the bar is in hand.
    pub hand: usize,
}

impl Default for Pack {
    fn default() -> Self {
        Pack { slots: [None; SLOTS], hand: 0 }
    }
}

impl Pack {
    /// What a person sets out with: a torch, a short sword, a shovel and a handful of berries.
    pub fn starting() -> Self {
        let mut pack = Pack::default();
        for (item, count) in [(Item::Sword, 1), (Item::Torch, 1), (Item::Shovel, 1), (Item::Berries, 6)] {
            pack.add(item, count);
        }
        pack
    }

    /// The thing in hand.
    pub fn held(&self) -> Option<Item> {
        self.slots[self.hand].map(|s| s.0)
    }

    /// Takes a slot of the bar in hand.
    pub fn hold(&mut self, slot: usize) {
        self.hand = slot.min(BAR - 1);
    }

    /// The next slot of the bar along (or back), round the end.
    pub fn cycle(&mut self, by: i32) {
        self.hand = (self.hand as i32 + by).rem_euclid(BAR as i32) as usize;
    }

    /// Puts some in: onto stacks of the same first, then into empty slots. Says how many would
    /// not fit.
    pub fn add(&mut self, item: Item, mut count: u32) -> u32 {
        for pass in 0..2 {
            for slot in self.slots.iter_mut() {
                if count == 0 {
                    return 0;
                }
                match slot {
                    Some((held, n)) if pass == 0 && *held == item && *n < item.stack() => {
                        let fits = (item.stack() - *n).min(count);
                        *n += fits;
                        count -= fits;
                    }
                    None if pass == 1 => {
                        let fits = item.stack().min(count);
                        *slot = Some((item, fits));
                        count -= fits;
                    }
                    _ => {}
                }
            }
        }
        count
    }

    /// Takes one from a slot; says what it was.
    pub fn take(&mut self, slot: usize) -> Option<Item> {
        let (item, n) = self.slots[slot]?;
        self.slots[slot] = (n > 1).then_some((item, n - 1));
        Some(item)
    }

    /// Changes two slots over; two stacks of the same thing are put together.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        if let (Some((ia, na)), Some((ib, nb))) = (self.slots[a], self.slots[b])
            && ia == ib
        {
            let moved = (ia.stack() - nb).min(na);
            self.slots[b] = Some((ib, nb + moved));
            self.slots[a] = (na > moved).then_some((ia, na - moved));
            return;
        }
        self.slots.swap(a, b);
    }

    pub fn count(&self, item: Item) -> u32 {
        self.slots.iter().flatten().filter(|s| s.0 == item).map(|s| s.1).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_sets_out_with_a_torch_and_a_weapon() {
        let pack = Pack::starting();
        assert_eq!(pack.held(), Some(Item::Sword));
        assert!(pack.count(Item::Torch) == 1 && pack.count(Item::Berries) == 6);
        // All of it is on the bar, to hand.
        assert!(pack.slots[BAR..].iter().all(Option::is_none));
    }

    #[test]
    fn things_stack_fill_and_run_out() {
        let mut pack = Pack::default();
        assert_eq!(pack.add(Item::Berries, 45), 0);
        assert_eq!((pack.slots[0], pack.slots[1], pack.slots[2]), (Some((Item::Berries, 20)), Some((Item::Berries, 20)), Some((Item::Berries, 5))));
        assert_eq!(pack.add(Item::Sword, SLOTS as u32), 3, "a sword to a slot, and three slots were taken");
        assert_eq!(pack.take(2), Some(Item::Berries));
        assert_eq!(pack.count(Item::Berries), 44);
        for _ in 0..4 {
            pack.take(2);
        }
        assert_eq!((pack.slots[2], pack.take(2)), (None, None));
    }

    #[test]
    fn slots_change_over_and_the_hand_goes_round_the_bar() {
        let mut pack = Pack::starting();
        pack.swap(0, 10);
        assert_eq!((pack.slots[0], pack.slots[10]), (None, Some((Item::Sword, 1))));
        assert_eq!(pack.held(), None);
        pack.cycle(-1);
        assert_eq!(pack.hand, BAR - 1);
        pack.cycle(2);
        assert_eq!(pack.held(), Some(Item::Torch));
        // Two part stacks go together.
        let mut pack = Pack::default();
        (pack.slots[0], pack.slots[1]) = (Some((Item::Berries, 15)), Some((Item::Berries, 12)));
        pack.swap(0, 1);
        assert_eq!((pack.slots[0], pack.slots[1]), (Some((Item::Berries, 7)), Some((Item::Berries, 20))));
    }
}
