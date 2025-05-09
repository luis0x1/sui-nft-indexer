-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateTable
CREATE TABLE "object_types" (
    "id" VARCHAR(1000) NOT NULL,
    "fields" TEXT NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "object_types_pkey" PRIMARY KEY ("id")
);
