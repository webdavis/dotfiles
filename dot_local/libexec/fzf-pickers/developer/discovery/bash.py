import re

import developer.projects as project_paths


def bash_tests(path):
    result = []
    for number, line in enumerate(project_paths.read_text(path).splitlines(), 1):
        match = re.match(r"^\s*(?:function\s+)?(test_[A-Za-z0-9_]+)\s*\(\s*\)\s*\{", line)
        if match:
            result.append((match[1], number))
    return result


def bash_test_command(path, line):
    return ["bashunit", f"{path}:{line}"]
