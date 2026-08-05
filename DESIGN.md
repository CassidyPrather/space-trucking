**Space Trucking** is a game where players transport cargo across space. It's meant to be played in the background. In the long run, it's designed for social gaming platforms (VRChat, etc.) as a less obtrusive backdrop to hangout nights.

This is a **prototype**, so things will be iterated upon rapidly and may not fully conform to the design. Once something sticks, it will probably be ported to VRChat.

Contact:
- cassidy@wirenook.net
- birb@cor.gg

Don't try to implement all the design stuff at once, it's more brainstorming than anything else.

# Lore

Cor is the one who wrote most of this stuff, not me, so I'll summarize:

Sometime in the distant future, humanity is space-faring. The "Inner Ring" (Venus, Earth, and Mars; Mercury is too hot) planets are mostly exhausted and matter extraction and processing has continued development on the nascent frontier of the "Outer Ring" (Jupiter, Uranus, Neptune). Each of the three factions of the inner ring aren't great at getting along, though, so freight between each other and the outer ring is difficult.

That's where the "Spacing Guild" comes in- headquartered in an unknown location in the Outer Ring, they operate in shady legal areas and handle shipping contracts. The player is one such contractor.

There are mysteries abound, allegedly, and that should be the "hook" for the lore- basically, provide as few details as possible. Unfortunately, Cor hasn't even told *me* what they are...

(In future passes, we may add some warp-y shenanigans (there are a million sci-fi ways to do this to pick from) and add more star systems, but this is a solid start)

## Tidbits

Venus is where the rich went to live. They are unimaginably tacky.

Earth is a dystopia of some sort. Pick a creative one.

Mars broke off in a rebellion and are now a scrappy republic.

Worlds in the outer ring probably need their own lore, to be written. A
start (prototype canon, revise freely):

- **Saturn** is the planet the outer-ring roster never mentions, and the
  ring-barons like it that way. The rings are a debris field of a thousand
  failed hauling companies, ground fine and picked over; salvage is the
  whole economy, scrap trades like treasure, and every docking clamp on the
  station was pulled off a repossessed freighter — including, possibly,
  yours.
- **The Umbra Market** floats in Mercury's shadow (Mercury itself is too
  hot, but its shadow is prime real estate) and only answers hails while
  the *caller's* clock reads deep night, which should not be possible and
  is not explained. They bottle midnight and sell it. They pay extra for
  rat-gnawed goods — "aged in transit, artisanal."
- **The Hermitage** is a hollowed rock in the asteroid belt. The hermits
  do not trade with strangers; they remember gifts, forever, and shelves
  slowly grow things for people who gave first. Nobody has seen more than
  one lit window.
- **The comet** has no name on any chart. When it dives near the sun,
  people go chip ice off it, because it is there and the ice is free.
  Sometimes there is something else in the ice.
- **???** is only there when three mysterious crates hum in a hold at
  once. It trades three for one bigger one. The Guild counts the bigger
  one four times. Nobody explains the arithmetic, least of all Cor.

# Aesthetic

3D first person (no avatar in the prototype).

Should be **easy to work with and iterate on** and **cheap on the GPU**.

- Low polygon-count geometry
- Textures with "smoothing" off (i.e. hard pixel edges)
- Small textures, tiling as needed, or using swatches
- Good lighting (thinking of [light volumes](https://github.com/REDSIM/VRCLightVolumes)

We're not set on an aesthetic yet, but some moodboard-level stuff includes:
- Treasure Planet & Outer Wilds
- Submarine-esque

No matter what, the environment of the game should feel kind of junky and scrappy. In the first pass minimum viable product (or whatever we're calling it) an "enclosed box with flavor" is good enough.

Research existing asset libraries that fit this bill for the prototypes.

There's necessarily going to be a difference in art direction between the prototype (which must be "easy" to implement and modify) and the final product.

Ambient noises and sound design, but no music. Big inspiration: https://www.gridsagegames.com/blog/2020/06/building-cogminds-ambient-soundscape/

# Mechanics

The core game should be extremely simple:

- Player selects a point of interest
- Wait a while until the ship arrives
- Barter (no currency allowed!!!)

Complexity emerges from the cargo itself, events, major modules, and social interaction.

## Cargo

- Cargo pieces double as cosmetics; i.e., everything should be physically modeled out, and nothing is strictly *required* to be delivered
- Cargo **should** have placement restrictions
- Cargo **may** have other behaviors
- Cargo **should** have special interactions with other items
- Cargo **should** have variants, e.g. in terms of color, size, and etcetera.
- Cargo **should** be easy to render
- Cargo **must not** have physics (that would make networking and performance a pain)
- Cargo **should** help tell the story of the lore (this is a text and dialogue free game, after all)
- Cargo **should** be interesting, not boring
- There must be some gameplay mechanic to obscure the technical limitation of "we can't render all this junk at the same time"

Design cargo with intent! Messing with cargo is going to be the main thing to do in transit.

# Multiplayer

When people join a game together, they should all "crew" the same ship. To facilitate drop-in drop-out accessibility, players shall own elements of the ship that are added when they connect, and disappear when they leave.

When a player connects to the instance, they "attach" their area to the ship somehow. Most of the ship should consist of player areas. I'm not sure topologically how that's going to work. It doesn't need to be realistic, either: If player A leaves, the "door" to their section can disappear, and when player B joins, their door should still be in the same place somehow.

Each player had one **major module**. Examples: Engine, expanded cargo bay, crafting station, etcetera.

The attached areas should be as high traffic as possible, both in terms of their positioning and mechanical relevance. In turn, the "generic" parts of the ship should be boring and off to the side (to the extent that they're even necessary). It'd be nice, but not strictly necessary, to design a topology that can help take advantage of visual and audio culling

"When somebody leaves, their module doesn't have to disappear immediately, it can just be de-activated" - Cor (though I don't see why it matters)

While some anti-griefing is probably necessary, I'd rather be light on it. Players should be able to barter with each other

Let's assume an absolute maximum player count of 6 for now, but expect to raise that, not lower it, as time goes on.

## Major Modules

When designing these, think:
- How is this social? e.g. Engine design implies deciding with crew what cargo to incinerate
- How does the game play without it? None of these should be necessary
- How might this be interesting over a long period of time?
- How does this interact with other modules?
- How does this interact with the story

We should have very few major modules- after all, players should only have one.


Examples:
- Navigation suite: Chart routes between POIs and get up-to-date data on the economy to make informed decisions with (instead of flying blind). Maybe could include a radio mini-game (I liked the one in high-fleet and think something with big tactile components would translate great to VR)
- Engine: Cargo can be thrown into the engine to generate a temporary speed boost. The more flammable the cargo, the bigger the speed boost. Flammability is not a listed metric, but every bit of cargo has it; kind of go by vibes.
- Exterior arm: Grabs stuff in space. 'nuff said, translates well to VR.
- Hallway: "Instead of pushing other player's modules out further, a hallway module connects more of the rooms tighter together" - Cor (too technically complex for me, I dunno, topology might be whacky)

## Central Server

Players shall be able to contribute towards one or more "global objectives"- this is a good sink for all that expensive (computationally and otherwise) cargo!

For example, the spacing guild has a *massive*, inexplicable hangar. As players deliver Suspicious Cargo (some items hum), they might notice that the spacing guild will *immediately* steal the cargo in front of the usual bartering minigame, and that cargo will get shuttled off into the hanger. There should be some visual indicator on the hangar of global progress.

Don't be too heavy-handed with the central server stuff, though.

... I'm kind of inspired by https://archipelago.gg/...

# Events

"Events" should occur in the game. Design guidelines:

- Must not be *wholly* arbitrary, though trigger conditions may involve chance
- Must be feasible to ignore (disengagement is participation)

Example trigger conditions:

- Module present
- Module combo present
- Cargo type present
- Cargo distribution
- Cargo bay full
- Cargo bay empty

Example events:

- Flying through the asteroid belt causes hull breaches (for some reason the atmosphere isn't pressurized, but the breaches are ugly and cause some minor inconvenience until repaired)
- There are rats on the ship which will damage the cargo (i.e. requiring repair) unless dealt with
- Player has a secret objective to color-code the cargo
- A cargo mimic appears in an under-scrutinized area
- When Suspicious Cargo is present, sometimes the lights in the ship go dim and the "hum" of suspicious cargo becomes global; when the lights come back on, the ship should have teleported closer to its destination
- Ad bots (flying annoying spaced billboards) need to be shot down (or batted away with the robot arm, or whatever)

# Guidance

The `game-template` repo should have some great general principles. These came to mind explicitly when scoping.

- Whimsy is the #1 requirement
- The gme must feature **absolutely no text or dialogue**. When you can't do that, make sure it's translatable.
- The game **absolutely must be deterministic and tolerant of network failures**. Allow quick catchup, fast-forward, pausing, saving, the works. These should be top priority to test **at all times**.
- We don't care about anti-cheat (if there ends up being a central server, anti-cheat must be confined to that)
- Come up with some sort of system for intermittent design review checklists to make sure we're not getting distracted from our core goals
- Do not allow for significant long-term progression mechanically. Players shouldn't get "upgrades". Maybe they can get more skilled at making decisions, or get a shinier collection of cargo, but that's it.
- Keep the space cramped and don't require too much player locomotion to play.
- Maximum download size: 85MB (excluding lazy-loaded assets)
- Come up with some other profiling metrics to enforce
- Prioritize telemetry (and figure out how to get informed consent before letting people play)
- Don't worry too much about forwards/backwards compatibility during the prototyping phase
- Lean so, so heavily on CI and automated tests
- Juice the bartering minigame, but please try not to design it in such a way that expensive 3D models are necessary
- POIs don't need to *exclusively* be planets, they can be moons, space stations, weird extra stuff...
- Each "star system" will be its own genre- for example, the game's aesthetic could be sailing instead of spaceships in star system 2. In the spirit of whimsy, don't think too hard about why the spaceships are also sailing ships just fine all of the sudden
- The game should "keep running" even if you close it. It's deterministic and should be easy to fast forward, after all!

# Required Reading

1. [Objects in Space](https://store.steampowered.com/app/824070/Objects_in_Space/)
2. [Bus to Nowhere](https://vrclist.com/world/6395)
3. [FISH!](https://vrclist.com/world/1187941)
4. [Monument](https://vrclist.com/world/995674)

## Cor's Required Reading

I will be real, she hasn't even told me all the stuff she wants- this is one of the few written things about this game I've been able to pry from her. I think the LLM she was chatting with recommended these to her:

• Digital: A Love Story — a primary inspiration; BBS interface and AI mystery.
• Hypnospace Outlaw — learning people through messy traces in an alternate old internet.
• The Beginner’s Guide — trying to understand a person through what they made.
• Serial Experiments Lain — identity, selfhood, and networks.
• Emily Is Away — early-2000s chat texture and nostalgia.
• Night in the Woods — friendships, drifting away, and returning.
• Outer Wilds — learning absent people through traces in the world.
• Return of the Obra Dinn — cross-checking accounts and building confidence from evidence.
• Disco Elysium — distinct internal voices with different agendas and reliability.
• The Stanley Parable — the player pushing against an authored system.
• Haruhi, K-On!, and Hyouka — clubs as small social worlds and story engines.

# Scratchpad

Separation of Concerns vs. Locality of Behavior: If we have, say, an engine, all the stuff related to the engine should be in the engine directory...

But that doesn't work if you optimize and share resources, as many things will end up pointing to the same shared stuff.
