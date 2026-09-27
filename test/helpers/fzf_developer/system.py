import json

import developer as dev
from developer.agents import context
from developer.system import processes, services


def test_process_connections(cwd, state):
    process = processes.process_rows(
        " 12 1 me /usr/bin/server\n 13 1 me /bin/worker",
        "p12\ncserver\nf4\nn*:8080\nTST=LISTEN\n",
    )
    assert process[0]["value"] == "12" and "8080" in process[0]["detail"]
    assert len(process) == 2


def test_container_details_and_shell(cwd, state):
    card = services.container_rows(
        [
            {
                "ID": "abc",
                "Names": "web",
                "State": "running",
                "Image": "app:1",
                "Environment": {"PASSWORD": "never show"},
            }
        ],
        cwd,
    )[0]
    assert "PASSWORD" not in card["detail"] and "never show" not in card["detail"]
    assert dev.accept("services", [card], "alt-e", state)["argv"] == [
        "docker",
        "exec",
        "-it",
        "abc",
        "sh",
    ]


def test_shell_provenance(cwd, state):
    provenance = cwd / "provenance.json"
    provenance.write_text(
        json.dumps(
            [
                {
                    "name": "ll",
                    "kind": "alias",
                    "detail": "ll is aliased to `ls -l'",
                    "value": "ll",
                }
            ]
        )
    )
    assert (
        dev.collect("provenance", dict(state, provenance_file=str(provenance)))[0]["value"] == "ll"
    )


def test_source_excerpts(cwd, state):
    source = cwd / "source.py"
    source.write_text("one\ntwo\n")
    excerpts = context.excerpt_rows([source], cwd)
    assert excerpts[0]["value"] == "source.py:1\none\ntwo"
    assert excerpts[0]["line"] == 1
