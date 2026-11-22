from pathlib import Path
import sys

from m7_public_cases import verify


def program(functions, expression):
    return functions + "\nfn main()->int{ return " + expression + " }"


leaf = "async fn leaf()->int{ return 42 }"
verify(Path(sys.argv[1]).resolve(strict=True), [
    (program(leaf, "await leaf()"), 42),
    (program(leaf + "\nasync fn parent()->int{ return await leaf() }", "await parent()"), 42),
    (program("async fn leaf()->int{ return 9223372036854775807 + 1 }", "await leaf()"), "runtime"),
    (program(leaf, "leaf()"), "reject"),
    (program("fn leaf()->int{ return 42 }", "await leaf()"), "reject"),
])

shared_job = 'share x:str = "A"\n' + leaf
before = shared_job + "\nfn main()->int{\nconst a:str = x.borrow()\n"
verify(Path(sys.argv[1]).resolve(strict=True), [
    (before + "const k:int = a[0][0]\nreturn await leaf()\n}", 42),
    (before + "const k:int = await leaf()\nreturn a[0][0]\n}", "reject"),
])
