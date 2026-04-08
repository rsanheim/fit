RSpec.describe "git-all passthrough" do
  it "preserves full git output for passthrough commands" do
    repo_a = create_repo("alpha")
    repo_b = create_repo("beta")

    File.write(File.join(repo_a, "alpha.txt"), "alpha\n")
    git(repo_a, "add", "alpha.txt")
    git(repo_a, "commit", "-m", "alpha-commit")

    File.write(File.join(repo_b, "beta.txt"), "beta\n")
    git(repo_b, "add", "beta.txt")
    git(repo_b, "commit", "-m", "beta-commit")

    expected_stdout = +""
    expected_stderr = +""

    ["alpha", "beta"].each do |name|
      repo = File.join(@workspace, name)
      stdout, stderr, status = Open3.capture3("git", "-C", repo, "log", "--oneline", "-1")
      unless status.success?
        raise "git -C #{repo} log --oneline -1 failed:\nstdout: #{stdout}\nstderr: #{stderr}"
      end
      expected_stdout << stdout
      expected_stderr << stderr
    end

    result = run_git_all("log", "--oneline", "-1")
    expect(result.stdout).to eq(expected_stdout)
    expect(result.stderr).to eq(expected_stderr)
  end

end
