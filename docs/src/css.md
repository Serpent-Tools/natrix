# Css

> [!NOTE]
> Natrix's css bundling system requires the use of the natrix cli.
> As such css bundling will not work when embedding natrix in other frameworks.

Natrix uses a unique css bundling system that allows for css to be declared in rust files, but bundled at compile time.
This is very different from other rust frameworks, which either do runtime injection, or require static external css files. Both of which have downsides that natrix solves.

The main advantage of this design is that css for dependencies is bundled along with the code on crates.io and is automatically combined with your own at **compile time**.

