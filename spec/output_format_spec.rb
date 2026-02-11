RSpec.describe "git-all output format" do
  context "pipe-delimited format" do
    it "outputs three columns separated by pipes" do
      create_repo("repo-a")
      create_repo("repo-b")

      result = run_git_all("status")
      lines = result.stdout.lines.map(&:chomp).reject(&:empty?)

      lines.each do |line|
        parts = line.split("|")
        expect(parts.length).to eq(3), "Expected 3 columns in: #{line.inspect}"
      end
    end
  end

  context "column alignment" do
    it "has pipe characters at the same positions across all rows" do
      create_repo("short")
      create_repo("a-longer-repo-name")
      create_repo("mid-length")

      result = run_git_all("status")
      lines = result.stdout.lines.map(&:chomp).reject(&:empty?)

      first_pipe_positions = lines.map { |l| l.index("|") }
      expect(first_pipe_positions.uniq.length).to eq(1),
        "First pipe positions differ: #{first_pipe_positions}"

      second_pipe_positions = lines.map { |l| l.index("|", l.index("|") + 1) }
      expect(second_pipe_positions.uniq.length).to eq(1),
        "Second pipe positions differ: #{second_pipe_positions}"
    end
  end

  context "alphabetical ordering" do
    it "sorts repos alphabetically by name" do
      create_repo("zeta-repo")
      create_repo("alpha-repo")
      create_repo("mid-repo")

      result = run_git_all("status")
      rows = parse_output(result.stdout)
      repo_names = rows.map(&:repo)

      expect(repo_names).to eq(repo_names.sort)
      expect(repo_names.first).to eq("alpha-repo")
      expect(repo_names.last).to eq("zeta-repo")
    end
  end

  context "truncation" do
    it "truncates long repo names with trailing ellipsis" do
      long_name = "this-is-an-extremely-long-repository-name-that-should-be-truncated"
      create_repo(long_name)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = rows.first
      expect(row.repo).to end_with("...")
      expect(row.repo.length).to be < long_name.length
    end
  end

  context "no repositories found" do
    it "prints a message and exits with code 9" do
      Dir.mktmpdir("empty-workspace-") do |empty_dir|
        result = run_git_all("status", dir: empty_dir)

        expect(result.stdout).to include("No git repositories found")
        expect(result.exit_code).to eq(9)
      end
    end
  end
end
