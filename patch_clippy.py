import sys

def modify():
    with open('src/strings/mini_parser.rs', 'r') as f:
        content = f.read()

    search = """    if let Some(&'-') = iter.peek() {
        sign = -1;
        iter.next();
    }"""
    replace = """    if iter.peek() == Some(&'-') {
        sign = -1;
        iter.next();
    }"""

    if search in content:
        content = content.replace(search, replace)
        with open('src/strings/mini_parser.rs', 'w') as f:
            f.write(content)
        print("Success for mini_parser.rs clippy fix")

modify()
