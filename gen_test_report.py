with open("test_results.txt", "r") as f:
    vs = f.read()
    
    tests = {}
    parts = vs.split("\n\n")
    
    for summary in parts[0].split("\n"):
        if not summary[:4] == "test":
            continue
        
        name, outcome = summary[5:].split("...")
        name = name.strip()
        outcome = outcome.strip()
        
        failed = (outcome == "FAILED")
        passed = (outcome == "ok")
        
        tests[name] = {"failed": failed, "passed": passed}
    
    parts = parts[1:]
    for part in parts:
        if not part[:4] == "----":
            continue
        lines = part.split("\n")
        name = lines[0].split(" ")[1]
        lines = lines[1:]
        
        tests[name]["output"] = "\n".join(lines)

total = len(tests)
passed = len([t for t in tests.values() if t["passed"]])
failed = len([t for t in tests.values() if t["failed"]])
skipped = total - passed - failed

with open("test_results.xml", "w") as f:
    f.write("<assemblies>\n")
    f.write(f"\t<assembly total={total} passed={passed} failed={failed} skipped={skipped}>\n")
    f.write(f"\t\t<collection total={total} passed={passed} failed={failed} skipped={skipped}>\n")
    
    for name, test in tests.items():
        result = "Skip"
        if test["passed"]:
            result = "Pass"
        if test["failed"]:
            result = "Fail"
        f.write(f"\t\t\t<test name=\"{name}\" result=\"{result}\">\n")
        
        if test["failed"]:
            f.write("\t\t\t\t<failure>\n")
            f.write(f"\t\t\t\t\t<message>{test['output']}</message>\n")
            f.write("\t\t\t\t</failure>\n")
        
        f.write("\t\t\t</test>\n")
    
    f.write("\t\t</collection>\n")
    f.write("\t</assembly>\n")
    f.write("</assemblies>")