module GitAll
  VERSION = {{ `shards version "#{__DIR__}"`.chomp.stringify }}
end

DEFAULT_WORKERS = 8
