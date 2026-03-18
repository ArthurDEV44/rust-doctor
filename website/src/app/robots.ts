import type { MetadataRoute } from "next";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: [
      // Allow everything by default
      { userAgent: "*", allow: "/" },

      // === OpenAI ===
      { userAgent: "GPTBot", allow: "/" },
      { userAgent: "ChatGPT-User", allow: "/" },
      { userAgent: "OAI-SearchBot", allow: "/" },

      // === Anthropic ===
      { userAgent: "ClaudeBot", allow: "/" },
      { userAgent: "anthropic-ai", allow: "/" },
      { userAgent: "Claude-Web", allow: "/" },
      { userAgent: "Claude-User", allow: "/" },
      { userAgent: "Claude-SearchBot", allow: "/" },

      // === Google AI ===
      { userAgent: "Googlebot", allow: "/" },
      { userAgent: "Google-Extended", allow: "/" },
      { userAgent: "Google-CloudVertexBot", allow: "/" },
      { userAgent: "GoogleOther", allow: "/" },
      { userAgent: "GoogleOther-Image", allow: "/" },
      { userAgent: "GoogleOther-Video", allow: "/" },
      { userAgent: "Gemini-Deep-Research", allow: "/" },
      { userAgent: "GoogleAgent-Mariner", allow: "/" },
      { userAgent: "Google-NotebookLM", allow: "/" },

      // === Microsoft / Bing ===
      { userAgent: "Bingbot", allow: "/" },
      { userAgent: "AzureAI-SearchBot", allow: "/" },

      // === Perplexity ===
      { userAgent: "PerplexityBot", allow: "/" },
      { userAgent: "Perplexity-User", allow: "/" },

      // === Meta / Facebook ===
      { userAgent: "FacebookBot", allow: "/" },
      { userAgent: "facebookexternalhit", allow: "/" },
      { userAgent: "meta-externalagent", allow: "/" },
      { userAgent: "Meta-ExternalAgent", allow: "/" },
      { userAgent: "meta-externalfetcher", allow: "/" },
      { userAgent: "Meta-WebIndexer", allow: "/" },

      // === Apple ===
      { userAgent: "Applebot", allow: "/" },
      { userAgent: "Applebot-Extended", allow: "/" },

      // === xAI / Grok ===
      { userAgent: "Grok", allow: "/" },
      { userAgent: "GrokBot", allow: "/" },
      { userAgent: "xAI-Bot", allow: "/" },

      // === Amazon / AWS ===
      { userAgent: "Amazonbot", allow: "/" },
      { userAgent: "Amzn-SearchBot", allow: "/" },
      { userAgent: "bedrockbot", allow: "/" },

      // === Cohere ===
      { userAgent: "cohere-ai", allow: "/" },
      { userAgent: "cohere-training-data-crawler", allow: "/" },

      // === ByteDance / TikTok ===
      { userAgent: "Bytespider", allow: "/" },
      { userAgent: "TikTokSpider", allow: "/" },

      // === Mistral AI ===
      { userAgent: "MistralAI-User", allow: "/" },

      // === DeepSeek ===
      { userAgent: "DeepSeekBot", allow: "/" },

      // === Huawei ===
      { userAgent: "PanguBot", allow: "/" },

      // === AI Search Engines ===
      { userAgent: "YouBot", allow: "/" },
      { userAgent: "DuckAssistBot", allow: "/" },
      { userAgent: "Bravebot", allow: "/" },
      { userAgent: "Brightbot", allow: "/" },
      { userAgent: "kagi-fetcher", allow: "/" },
      { userAgent: "AndiBot", allow: "/" },
      { userAgent: "PhindBot", allow: "/" },
      { userAgent: "iAskBot", allow: "/" },
      { userAgent: "iaskspider", allow: "/" },
      { userAgent: "TavilyBot", allow: "/" },
      { userAgent: "LinkupBot", allow: "/" },
      { userAgent: "ExaBot", allow: "/" },
      { userAgent: "ZanistaBot", allow: "/" },
      { userAgent: "WRTNBot", allow: "/" },
      { userAgent: "TimpiBot", allow: "/" },

      // === Common Crawl (AI training datasets) ===
      { userAgent: "CCBot", allow: "/" },

      // === AI Research / Open Source ===
      { userAgent: "AI2Bot", allow: "/" },
      { userAgent: "Ai2Bot-Dolma", allow: "/" },
      { userAgent: "HuggingFace-Bot", allow: "/" },

      // === Cloud / Infrastructure AI ===
      { userAgent: "DigitalOceanGenAICrawler", allow: "/" },
      { userAgent: "Cloudflare-AutoRAG", allow: "/" },

      // === China Search ===
      { userAgent: "Baiduspider", allow: "/" },
      { userAgent: "ChatGLM-Spider", allow: "/" },
      { userAgent: "YandexAdditional", allow: "/" },
      { userAgent: "PetalBot", allow: "/" },

      // === SEO / Data AI ===
      { userAgent: "SemrushBot", allow: "/" },
      { userAgent: "AhrefsBot", allow: "/" },
      { userAgent: "Diffbot", allow: "/" },

      // === Agentic AI ===
      { userAgent: "Manus-User", allow: "/" },
      { userAgent: "FirecrawlAgent", allow: "/" },
      { userAgent: "ApifyBot", allow: "/" },

      // === SoftBank ===
      { userAgent: "SBIntuitionsBot", allow: "/" },
    ],
    sitemap: "https://rust-doctor.dev/sitemap.xml",
  };
}
