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
```
