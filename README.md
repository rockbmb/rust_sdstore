# SDStore in Rust

This repository contains a solution to an Operating Systems project from a class offered by
the [Department of Informatics](https://di.uminho.pt/) of the university I studied in.

This solution is in Rust; however, for this class, we were restricted to using C and UNIX syscall
wrappers in e.g. `unistd.h, fcntl.h, sys/stat.h`.

The report for our previous solution is located [here](https://github.com/Alef-Keuffer/SDStore/blob/main/report/relprojLayout.pdf)
-- sadly it's in Portuguese, but in this document I hope to present the architecture of this solution,
and when relevant, compare it to the previous one we developed.

# Outline of the problem

For this project, it was requested that students implement a [daemon](https://en.wikipedia.org/wiki/Daemon_(computing))
which allows a user to submit compression and encryption requests on files of their choosing.
The system is to operate locally without network access, and must consist of two separate components:
* A server `sdstored` that will collect incoming requests, concurrently process them, and inform clients of the statu
  of their requests
* A client executable `sdstore`, which allows clients to submit requests and stay informed of their status, and to query
  the server on the state of its requests.

## File transformations

The allowed file transformation operations, interchangeably referred to as *filters*, are:

* `bcompress/bdecompress`: uses `bzip2` to (de)compress files
* `gcompress/gdecompress` uses `gzip` to (de)compress files
* `encrypt/decrypt`: uses `ccrypt` to encrypt/decrypt files
* `nop`: copies data via `cat`, doing nothing further

C source code, and binary executables for all of these filters must exist in a folder to which the server must be given
access upon startup.

In this repository's root there exists a `bin` folder with the required source code for these mock commands, and a `make`file to
build them with.
To test them, one can run:

```bash
$ (cd bin; make)
$ touch hi; echo "hello, friend" > hi
$ ./bin/bcompress < hi | ./bin/decompress > hello
$ cat hello
hello, friend
```

## Transformation limits, and server configuration

The server is restricted in the amount of **concurrent filters** it is allowed to run.
A configuration file specifying these limits is provided to the server upon startup as well.
An example of such a configuration follows:

```
nop 3
bcompress 4
bdecompress 4
gcompress 2
gdecompress 2
encrypt 2
decrypt 2
```

The above would mean that a server is allowed to run at most 3 concurrent instances of the `nop` filter,
**whether** in a single request, **or** in many.

E.g.:

* if one client submits a request consisting of a single `nop`, and another client a request consisting of just 2 `nop`s,
  they may be executed simultaneously;
* if the first client had instead submitted a request with 2 `nop`s, the two requests
  would not be concurrently executable.
  The one received first by the server would run, and after it ended, the second would begin.

## Interface and capabilities

* The server must be started thusly:
  `./sdstored <config-filename> <path-to-filters>`

* The client should:
  * Allow submission of requests via
    `./sdstore proc-file <priority> <input-file> <output-file> <filter>+`
    where `<filter>+` is a sequence of one or more filters, whose values have been enumerated [above](#file-transformations).
  * Return information on the server's currently pending and running tasks, and its running filter count:
    `./sdstore status`

    Example output for this command, taken from the course's statement, follows:
    ```
    task #3: proc-file 0 /home/user/samples/file-c file-c-output nop bcompress
    task #5: proc-file 1 samples/file-a file-a-output bcompress nop gcompress encrypt nop
    task #8: proc-file 1 file-b-output path/to/dir/new-file-b decrypt gdecompress
    transf nop: 3/3 (running/max)
    transf bcompress: 2/4 (running/max)
    transf bdecompress: 1/4 (running/max)
    transf gcompress: 1/2 (running/max)
    transf gdecompress: 1/2 (running/max)
    transf encrypt: 1/2 (running/max)
    transf decrypt: 1/2 (running/max)
    ```

