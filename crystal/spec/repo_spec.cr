require "./spec_helper"

def with_tempdir(&)
  tmpdir = File.join(Dir.tempdir, "fit-test-#{Random::Secure.hex(8)}")
  Dir.mkdir(tmpdir)
  begin
    yield tmpdir
  ensure
    FileUtils.rm_rf(tmpdir)
  end
end

describe Repo do
  describe ".discover" do
    it "returns empty array when no git repos found" do
      with_tempdir do |tmpdir|
        repos = Repo.discover(tmpdir)
        repos.should be_empty
      end
    end

    it "finds git repositories in subdirectories" do
      with_tempdir do |tmpdir|
        # Create a fake git repo
        repo_path = File.join(tmpdir, "test-repo")
        Dir.mkdir(repo_path)
        Dir.mkdir(File.join(repo_path, ".git"))

        repos = Repo.discover(tmpdir)
        repos.size.should eq(1)
        repos[0].should eq(repo_path)
      end
    end

    it "ignores directories without .git" do
      with_tempdir do |tmpdir|
        # Create a non-repo directory
        Dir.mkdir(File.join(tmpdir, "not-a-repo"))

        repos = Repo.discover(tmpdir)
        repos.should be_empty
      end
    end

    it "returns repos in sorted order" do
      with_tempdir do |tmpdir|
        # Create repos in reverse alphabetical order
        ["zebra", "alpha", "middle"].each do |name|
          repo_path = File.join(tmpdir, name)
          Dir.mkdir(repo_path)
          Dir.mkdir(File.join(repo_path, ".git"))
        end

        repos = Repo.discover(tmpdir)
        repos.size.should eq(3)
        repos.map { |r| File.basename(r) }.should eq(["alpha", "middle", "zebra"])
      end
    end
  end

  describe ".name" do
    it "extracts the repo name from a path" do
      Repo.name("/path/to/my-repo").should eq("my-repo")
    end
  end

  describe ".format_name" do
    it "pads short names to fixed width" do
      formatted = Repo.format_name("/path/to/short")
      formatted.size.should eq(24)
      formatted.should eq("short                   ")
    end

    it "truncates long names with ellipsis" do
      long_name = "this-is-a-very-long-repository-name"
      formatted = Repo.format_name("/path/to/#{long_name}")
      formatted.size.should eq(24)
      formatted.should end_with("...")
    end
  end
end
