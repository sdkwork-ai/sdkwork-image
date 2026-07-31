export interface AIImageOptions {
  prompt: string;
  negativePrompt?: string;
  aspectRatio: "1:1" | "16:9" | "9:16" | "4:3";
  style: string;
}

export interface ImageTask {
  id: string;
  options: AIImageOptions;
  status: "pending" | "generating" | "completed" | "failed";
  progress: number;
  imageUrl?: string;
  createdAt: number;
}

export class AIImageCapabilityUnavailableError extends Error {
  constructor(capability: "image generation" | "prompt optimization" | "image history") {
    super(`AI ${capability} is unavailable because no owner SDK is composed.`);
    this.name = "AIImageCapabilityUnavailableError";
  }
}

export class AIImageService {
  public static deleteFromHistory(_id: string): never {
    throw new AIImageCapabilityUnavailableError("image history");
  }

  public static async optimizePrompt(_prompt: string): Promise<string> {
    throw new AIImageCapabilityUnavailableError("prompt optimization");
  }

  public static async generateImage(
    _options: AIImageOptions,
    _onProgress?: (progress: number) => void,
  ): Promise<ImageTask> {
    throw new AIImageCapabilityUnavailableError("image generation");
  }

  public static async getHistory(): Promise<ImageTask[]> {
    throw new AIImageCapabilityUnavailableError("image history");
  }
}
