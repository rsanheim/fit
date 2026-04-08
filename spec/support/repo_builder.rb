require "open3"

module RepoBuilder
  def create_repo(name, branch: "main")
    path = File.join(@workspace, name)
    FileUtils.mkdir_p(path)
    git(path, "init", "-b", branch)
    git(path, "config", "user.email", "test@example.com")
    git(path, "config", "user.name", "Test User")
    # Create an initial commit so HEAD exists
    File.write(File.join(path, "README.md"), "# #{name}\n")
    git(path, "add", "README.md")
    git(path, "commit", "-m", "Initial commit")
    path
  end

  # Create a regular repo suitable for use as an upstream/remote.
  # Returns the path. The caller is responsible for cleanup.
  def create_upstream_repo(tmpdir_path)
    git_raw("init", "-b", "main", tmpdir_path)
    git(tmpdir_path, "config", "user.email", "test@example.com")
    git(tmpdir_path, "config", "user.name", "Test User")
    File.write(File.join(tmpdir_path, "README.md"), "# upstream\n")
    git(tmpdir_path, "add", "README.md")
    git(tmpdir_path, "commit", "-m", "Initial commit")
    tmpdir_path
  end

  def create_tracking_repo(name, upstream_path)
    path = File.join(@workspace, name)
    git_raw("clone", upstream_path, path)
    git(path, "config", "user.email", "test@example.com")
    git(path, "config", "user.name", "Test User")
    path
  end

  def with_tracking_repo(name)
    upstream = Dir.mktmpdir("upstream-")
    begin
      create_upstream_repo(upstream)
      repo = create_tracking_repo(name, upstream)
      yield repo, upstream
    ensure
      FileUtils.rm_rf(upstream)
    end
  end

  def add_modified_file(repo, filename = "modified.txt")
    filepath = File.join(repo, filename)
    File.write(filepath, "original content\n")
    git(repo, "add", filename)
    git(repo, "commit", "-m", "Add #{filename}")
    File.write(filepath, "modified content\n")
  end

  def add_staged_modification(repo, filename = "staged.txt")
    filepath = File.join(repo, filename)
    File.write(filepath, "original content\n")
    git(repo, "add", filename)
    git(repo, "commit", "-m", "Add #{filename}")
    File.write(filepath, "staged modification\n")
    git(repo, "add", filename)
  end

  def add_untracked_file(repo, filename = "untracked.txt")
    File.write(File.join(repo, filename), "untracked content\n")
  end

  def add_staged_new_file(repo, filename = "new-file.txt")
    File.write(File.join(repo, filename), "new file content\n")
    git(repo, "add", filename)
  end

  def stage_deletion(repo, filename = "to-delete.txt")
    filepath = File.join(repo, filename)
    File.write(filepath, "will be deleted\n")
    git(repo, "add", filename)
    git(repo, "commit", "-m", "Add #{filename}")
    git(repo, "rm", filename)
  end

  def unstaged_deletion(repo, filename = "will-vanish.txt")
    filepath = File.join(repo, filename)
    File.write(filepath, "will vanish\n")
    git(repo, "add", filename)
    git(repo, "commit", "-m", "Add #{filename}")
    FileUtils.rm(filepath)
  end

  def stage_rename(repo, old_name = "old-name.txt", new_name = "new-name.txt")
    filepath = File.join(repo, old_name)
    File.write(filepath, "will be renamed\n")
    git(repo, "add", old_name)
    git(repo, "commit", "-m", "Add #{old_name}")
    git(repo, "mv", old_name, new_name)
  end

  def detach_head(repo)
    sha = git(repo, "rev-parse", "HEAD").strip
    git(repo, "checkout", sha)
  end

  def make_ahead(repo, count)
    count.times do |i|
      File.write(File.join(repo, "ahead-#{i}.txt"), "ahead #{i}\n")
      git(repo, "add", "ahead-#{i}.txt")
      git(repo, "commit", "-m", "Ahead commit #{i}")
    end
  end

  # Create commits in the upstream that the clone doesn't have yet,
  # then fetch in the clone so it sees the behind state.
  def make_behind(upstream, clone, count)
    count.times do |i|
      File.write(File.join(upstream, "behind-#{i}.txt"), "behind #{i}\n")
      git(upstream, "add", "behind-#{i}.txt")
      git(upstream, "commit", "-m", "Behind commit #{i}")
    end
    git(clone, "fetch")
  end

  def git(repo, *args)
    stdout, stderr, status = Open3.capture3("git", "-C", repo, *args)
    unless status.success?
      raise "git -C #{repo} #{args.join(" ")} failed:\nstdout: #{stdout}\nstderr: #{stderr}"
    end
    stdout
  end

  def git_raw(*args)
    stdout, stderr, status = Open3.capture3("git", *args)
    unless status.success?
      raise "git #{args.join(" ")} failed:\nstdout: #{stdout}\nstderr: #{stderr}"
    end
    stdout
  end
end
