# `static_res!` macro

```
static_res! { "tests/**" }

fn main() {
	assert!(tests::test_txt == include_bytes!("../tests/test.txt"));
	assert!(tests::folder::test_txt == b"yet another test");
}
```
