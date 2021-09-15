# kaeshi

## usage

```bash
$ echo "name = aaa\nname = bbb" |  ./target/debug/kaeshi  -t "name = {{ title }}" -q 'SELECT title FROM main order by title'
+-------+
| title |
+=======+
| aaa   |
+-------+
| bbb   |
+-------+

$ curl -s https://www.rust-lang.org/ | ./target/debug/kaeshi -t '{{ ignore|skip }}<a href="{{ href }}" {{ rest|skip }}>Version {{ version }}</a>' -q 'SELECT href, version FROM kaeshi'
+---------+--------------------------------------------------------+
| version | href                                                   |
+=========+========================================================+
| 1.55.0  | https://blog.rust-lang.org/2021/09/09/Rust-1.55.0.html |
+---------+--------------------------------------------------------+
```
