# frozen_string_literal: true

require_relative "spec_helper"

RSpec.describe "nit" do
  describe "inside_git_repo?" do
    it "returns true when inside a git repo" do
      Dir.mktmpdir do |dir|
        Dir.chdir(dir) do
          system("git", "init", "--quiet", out: File::NULL, err: File::NULL)
          expect(inside_git_repo?).to be true
        end
      end
    end

    it "returns false when outside a git repo" do
      Dir.mktmpdir do |dir|
        Dir.chdir(dir) do
          expect(inside_git_repo?).to be false
        end
      end
    end
  end

  describe "find_repos" do
    it "finds repos at depth 1" do
      Dir.mktmpdir do |dir|
        Dir.chdir(dir) do
          %w[repo-a repo-b not-a-repo].each { |d| Dir.mkdir(d) }
          %w[repo-a repo-b].each { |d| Dir.mkdir("#{d}/.git") }

          repos = find_repos
          expect(repos).to eq(%w[repo-a repo-b])
        end
      end
    end

    it "returns repos sorted alphabetically" do
      Dir.mktmpdir do |dir|
        Dir.chdir(dir) do
          %w[zebra alpha middle].each do |name|
            Dir.mkdir(name)
            Dir.mkdir("#{name}/.git")
          end

          repos = find_repos
          expect(repos).to eq(%w[alpha middle zebra])
        end
      end
    end

    it "returns empty array when no repos found" do
      Dir.mktmpdir do |dir|
        Dir.chdir(dir) do
          Dir.mkdir("not-a-repo")
          expect(find_repos).to eq([])
        end
      end
    end
  end

  describe "format_repo_name" do
    it "pads short names to fixed width" do
      result = format_repo_name("foo")
      expect(result).to eq("[foo                     ]")
      expect(result.length).to eq(26) # brackets + 24 chars
    end

    it "handles exact width names" do
      name = "a" * MAX_REPO_NAME_WIDTH
      result = format_repo_name(name)
      expect(result).to eq("[#{name}]")
    end

    it "truncates long names with ellipsis" do
      name = "a" * 30
      result = format_repo_name(name)
      expect(result.length).to eq(26)
      expect(result).to end_with("-...]")
    end
  end

  describe "GitCommand" do
    describe "#to_cmd" do
      it "builds basic command array" do
        cmd = GitCommand.new("my-repo", ["status", "--porcelain"], nil)
        expect(cmd.to_cmd).to eq(["git", "-C", "my-repo", "status", "--porcelain"])
      end

      it "includes SSH config when url_scheme is :ssh" do
        cmd = GitCommand.new("my-repo", ["pull"], :ssh)
        argv = cmd.to_cmd
        expect(argv).to include("-c", "url.git@github.com:.insteadOf=https://github.com/")
        expect(argv.index("-c")).to be < argv.index("-C")
      end

      it "includes HTTPS config when url_scheme is :https" do
        cmd = GitCommand.new("my-repo", ["fetch"], :https)
        argv = cmd.to_cmd
        expect(argv).to include("-c", "url.https://github.com/.insteadOf=git@github.com:")
      end
    end

    describe "#to_s" do
      it "returns shell-escaped command string" do
        cmd = GitCommand.new("my-repo", ["status"], nil)
        expect(cmd.to_s).to eq("git -C my-repo status")
      end

      it "escapes paths with spaces" do
        cmd = GitCommand.new("my repo", ["pull"], nil)
        expect(cmd.to_s).to include("my\\ repo")
      end

      it "uses same argv as to_cmd (dry-run compliance)" do
        cmd = GitCommand.new("repo", ["fetch", "--all"], :ssh)
        # to_s should be the shell-joined version of to_cmd
        expect(cmd.to_s).to eq(cmd.to_cmd.shelljoin)
      end
    end
  end

  describe "FORMATTERS" do
    describe ":status" do
      let(:formatter) { FORMATTERS[:status] }

      it "returns 'clean' for empty porcelain output" do
        expect(formatter.call("", "", true)).to eq("clean")
      end

      it "counts modified files" do
        output = " M file1.rb\n M file2.rb\n"
        expect(formatter.call(output, "", true)).to eq("2 modified")
      end

      it "counts untracked files" do
        output = "?? new_file.rb\n?? another.rb\n"
        expect(formatter.call(output, "", true)).to eq("2 untracked")
      end

      it "counts staged files" do
        output = "M  staged.rb\nA  added.rb\n"
        expect(formatter.call(output, "", true)).to eq("2 modified")
      end

      it "combines multiple statuses" do
        output = " M modified.rb\n?? untracked.rb\n"
        expect(formatter.call(output, "", true)).to eq("1 modified, 1 untracked")
      end

      it "returns error message on failure" do
        expect(formatter.call("", "fatal: not a git repository\n", false))
          .to eq("ERROR: fatal: not a git repository")
      end
    end

    describe ":pull" do
      let(:formatter) { FORMATTERS[:pull] }

      it "returns 'Already up to date' when no changes" do
        expect(formatter.call("Already up to date.\n", "", true))
          .to eq("Already up to date")
      end

      it "extracts files changed summary" do
        output = <<~OUTPUT
          Updating abc123..def456
          Fast-forward
           README.md | 2 +-
           1 file changed, 1 insertion(+), 1 deletion(-)
        OUTPUT
        expect(formatter.call(output, "", true))
          .to eq("1 file changed, 1 insertion(+), 1 deletion(-)")
      end

      it "returns 'completed' when no summary found" do
        expect(formatter.call("Some other output\n", "", true)).to eq("completed")
      end

      it "returns error message on failure" do
        expect(formatter.call("", "error: failed\n", false))
          .to eq("ERROR: error: failed")
      end
    end

    describe ":fetch" do
      let(:formatter) { FORMATTERS[:fetch] }

      it "returns 'no new commits' when nothing fetched" do
        expect(formatter.call("", "", true)).to eq("no new commits")
      end

      it "returns 'fetched' when something fetched" do
        expect(formatter.call("", "From github.com:foo/bar\n", true)).to eq("fetched")
      end

      it "returns error message on failure" do
        expect(formatter.call("", "fatal: error\n", false))
          .to eq("ERROR: fatal: error")
      end
    end

    describe ":passthrough" do
      let(:formatter) { FORMATTERS[:passthrough] }

      it "returns first line of stdout" do
        expect(formatter.call("line 1\nline 2\n", "", true)).to eq("line 1")
      end

      it "falls back to stderr if stdout empty" do
        expect(formatter.call("", "warning: something\n", true)).to eq("warning: something")
      end

      it "returns 'ok' when both empty" do
        expect(formatter.call("", "", true)).to eq("ok")
      end

      it "returns error message on failure" do
        expect(formatter.call("", "error: something\n", false))
          .to eq("ERROR: error: something")
      end
    end
  end
end
