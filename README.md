# sver

Version calcurator based on source code.

## description

sver is small cli command for calucurate version based on source code of git repository.

## usage

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
