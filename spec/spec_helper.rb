require "tmpdir"
require "fileutils"

Dir[File.join(__dir__, "support", "**", "*.rb")].each { |f| require f }

RSpec.configure do |config|
  config.include GitAllRunner
  config.include RepoBuilder
  config.include OutputParser

  config.around(:each) do |example|
    Dir.mktmpdir("git-all-test-") do |workspace|
      @workspace = workspace
      example.run
    end
  end
end
