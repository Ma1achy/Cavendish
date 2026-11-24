# Adding a dev scene

**Example to read:** `staggered()` in `crates/cav-sim/src/dev.rs` and its file
`scenes/dev/staggered.ron`.

A dev scene is a scene whose answer is known, for building and testing one thing at a time
(`plan/REBUILD.md` §9.1). It exists twice on purpose: as a builder function, which tests call,
and as a `.ron` file, which people load and edit; a test holds the two together.

1. **The builder.** In `crates/cav-sim/src/dev.rs`, a function returning a `SceneSpec`, usually
   starting from an existing one (`let mut scene = comb();`). Its doc comment says what the scene
   is and what it checks. The scene's `name` is the name it is loaded by; its `description` is
   one or two sentences a person reads in the properties panel.
2. **Register it.** Add the function to the slice's list (`S3_SCENES`, `S2_SCENES`; a new slice adds its own
   and chains it in `the_s1_scene_files_are_their_builders`). The list is an array with its
   length in its type: raise the number with it.
3. **The file.** Add a line to `LIBRARY` in `crates/cav-sim/src/library.rs`
   (`scene!("my-scene", "scenes/dev/my-scene.ron")`), create the file with any content (the
   library embeds it at compile time, so it must exist), and let the test write it from the
   builder:

   ```
   CAV_REWRITE_SCENES=1 cargo nextest run -p cav-sim -E "test(the_s1_scene_files_are_their_builders)"
   ```

   Read the file it wrote. (`CAV_WRITE_SCENES=1` writes only files that do not exist; a file
   that exists is rewritten only with `CAV_REWRITE_SCENES`, and then every dev scene is, so read
   the whole diff.)
4. **Say what it is.** A few lines in `dev::tests::the_s1_dev_scenes_are_what_they_say`, or a
   test of its own, asserting the facts the scene is for.
5. **Use it.** The reason for a dev scene is usually a validation case: declare it beside the
   code it checks ([a-validation-case.md](a-validation-case.md)).

## Tests that then hold it

- `library::tests::scene_files_on_disk_are_the_built_in_library`: a file with no `LIBRARY` line,
  or the reverse, fails; so does a file whose name is not the scene's name.
- `dev::tests::the_s1_scene_files_are_their_builders`: the file is the builder's text.
- `cav`: `cli_lists_the_dev_scenes` counts the library: update the count.
- `console::tests::a_bad_line_says_what_was_expected_and_changes_nothing` quotes the start of
  the library's list in an error message; it changes only if you add near the start.

Followed end to end on 23 Nov 2026 with a made-up scene (`late-tower`, the comb with tower 2
half a second late): the builder, the list, the `LIBRARY` line, a file with one character in it,
the rewrite; `quick` then failed in exactly one place, `cli_lists_the_dev_scenes` (19 scenes, not
18), as said above. The other scene files did not change in the rewrite.

## Things that go wrong

- Editing the `.ron` file by hand and not the builder: the test fails and says which scene.
- A scene that takes long to read. `quick` loads every library scene; keep the timeline short
  (a few hundred shots) unless length is the point.
