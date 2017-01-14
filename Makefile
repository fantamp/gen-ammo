
all:
	cargo build


# You shoudl install cargo-local-pkgs to make this work:
#    $ cargo install cargo-local-pkgs
#    see https://github.com/jonas-schievink/cargo-local-pkgs for details
test:
	cargo local-pkgs test

clean:
	cargo local-pkgs clean

