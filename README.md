# sver

Version calcurator based on source code.

## Description

`sver` is small cli command for calucurate version based on source code of git repository.

## Usage

### Calcurate version of directory on git repository

Calcurate repository root.

```sh
$ sver calc .
ef5d3d3db6d5
```

Calcurate sub directory in the repository.

```sh
$ sver calc testdata/service1/
3f1bec06015e
```

Calcurate multiple directory and output toml format.

```sh
$ sver calc testdata/service1/ testdata/service2/ --output toml
[[versions]]
repository_root = "/home/mitoma/src/github.com/mitoma/sver/"
path = "testdata/service1"
version = "3f1bec06015e"

[[versions]]
repository_root = "/home/mitoma/src/github.com/mitoma/sver/"
path = "testdata/service2"
version = "fd0053eab4b8"
```

#### option

| name     | value                                   |
| -------- | --------------------------------------- |
| --length | hash length. short=12, long=64          |
| --output | output format. version-only, toml, json |

### List the source code used for hash calculation.

```
$ sver list testdata/service2
testdata/lib1/.gitkeep
testdata/lib2/sver.toml
testdata/service1/sver.toml
testdata/service2/sver.toml
...
```

### Validate the configuration files in the repository

```sh
sver validate
[OK]    /sver.toml:[default]
[OK]    testdata/cyclic1/sver.toml:[default]
[OK]    testdata/cyclic2/sver.toml:[default]
[NG]    testdata/invalid_config1/sver.toml:[default]
                invalid_dependency:["unknown/path"]
                invalid_exclude:[]
[NG]    testdata/invalid_config2/sver.toml:[default]
                invalid_dependency:[]
                invalid_exclude:["target"]
[OK]    testdata/lib2/sver.toml:[default]
[OK]    testdata/service1/sver.toml:[default]
[OK]    testdata/service2/sver.toml:[default]
```

## Config

By placing a `sver.toml` file, you can add dependent directories and files to the directory to be calculated.

`sver.toml` is a configuration file for defining directory dependencies.

| key                        | notes                                                                        |
| -------------------------- | ---------------------------------------------------------------------------- |
| \<profile\>                | Profile. default value is "default".                                         |
| \<profile\>.dependencies[] | Dependency files of directories. Set relative path from **repository root**. |
| \<profile\>.excludes[]     | Exclude files of directories.  Set relative path from **target directory**   |

**example1**

service1 depends on lib1 directory.

```sh
.
├── README.md
├── libs1
│   └── lib.rs
└── service1
     ├── main.rs
     └── sver.toml (1)
```

sver.toml (1)

```toml
[default]
# path from the root
dependencies = [
  "lib1",
]
excludes = []
```

**example2**

service1 ignore service1/doc directory.

```sh
.
├── README.md
├── libs1
│   └── lib.rs
└── service1
     ├── main.rs
     ├── sver.toml (2)
     └── doc
          └── design.md
```

sver.toml (2)

```toml
[default]
dependencies = [
  "lib1",
]
# path from the service1 directory
excludes = [
  "doc",
]
```

### profile support

If you want to switch between multiple source sets in version calculations, you can use profiles.

The profile is specified in the `<path>:<profile>` format.

ex) 
- `sver calc lib1:build`
- `sver calc .:default .:build`
- `sver list .:test`

**example3**

add build profile for ignore tests.

```sh
.
├── README.md
├── src
│   └── lib.rs
├── tests
│   └── test.rs
└── sver.toml (3)
```

sver.toml (3)

```toml
[default]
dependencies = []

[build]
excludes = ["README.md", "tests"]
```
